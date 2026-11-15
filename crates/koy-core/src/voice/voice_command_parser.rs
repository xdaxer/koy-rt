use super::path_util;
use super::voice_models::{ResolvedCommand, VoiceClaudePermission, VoiceCommandException, VoiceFailure, VoiceHarnessType};
use crate::text;

const BLOCKED_EXECUTABLES: [&str; 14] = [
    "cmd",
    "cmd.exe",
    "powershell",
    "powershell.exe",
    "pwsh",
    "pwsh.exe",
    "bash",
    "bash.exe",
    "sh",
    "sh.exe",
    "wscript",
    "wscript.exe",
    "cscript",
    "cscript.exe",
];

const DANGEROUS_SHELL_CHARACTERS: [char; 7] = ['|', '&', ';', '>', '<', '`', '$'];

/// Ayarda "model yok" anlamına gelen değer: araç kendi varsayılanını kullanır.
pub const DEFAULT_MODEL: &str = "default";

fn has_dangerous_shell_character(s: &str) -> bool {
    s.contains(DANGEROUS_SHELL_CHARACTERS)
}

/// Dikte edilen sesli komutu ve harness yapılandırmasını güvenli bir şekilde
/// ayrıştırarak yürütülebilir dosya ve argüman listesine dönüştürür.
/// ASLA kabuk (cmd.exe, powershell.exe) çalıştırmaz; doğrudan ArgumentList için bağımsız parçalar üretir.
pub struct VoiceCommandParser;

impl VoiceCommandParser {
    /// Harness türüne ve parametrelere göre yürütülecek komut ve argümanları çözer.
    pub fn resolve(
        harness_type: VoiceHarnessType,
        prompt: &str,
        custom_executable: Option<&str>,
        custom_arguments_template: Option<&str>,
        claude_permission: VoiceClaudePermission,
        model: Option<&str>,
    ) -> Result<ResolvedCommand, VoiceCommandException> {
        let safe_prompt = prompt;

        match harness_type {
            // "--" istemi seçenek olarak okunmasın diye konur: "-p --version" sürüm yazdırır, dikte "--version" olsa da istem olarak gider.
            // İzin bayrağı "--" öncesinde durur; sonrası istem sayılır.
            VoiceHarnessType::Claude => Ok(ResolvedCommand::new("claude", claude_arguments(safe_prompt, claude_permission, model)?)),
            VoiceHarnessType::Codex => {
                Ok(ResolvedCommand::new("codex", vec!["exec".to_string(), "--".to_string(), safe_prompt.to_string()]))
            }
            // agy (Go) -p değer alır ve sonraki argümanı tire ile başlasa da değer sayar; "--" gerekmez, konursa istem "--" olur.
            VoiceHarnessType::Agy => Ok(ResolvedCommand::new("agy", vec!["-p".to_string(), safe_prompt.to_string()])),
            VoiceHarnessType::Custom => resolve_custom(safe_prompt, custom_executable, custom_arguments_template),
        }
    }

    /// Özel argüman kalıbını tırnaklara duyarlı olarak parçalar ve {prompt} yer tutucusunu güvenle yerleştirir.
    /// {prompt} şablonda yoksa istem argüman listesinin sonuna bağımsız bir öğe olarak eklenir.
    pub fn parse_custom_arguments(template: Option<&str>, prompt: &str) -> Vec<String> {
        if text::is_null_or_white_space(template) {
            return vec![prompt.to_string()];
        }

        let tokens = tokenize(template.unwrap_or_default());
        build_arguments_from_tokens(&tokens, prompt)
    }
}

// Model adı seçenek gibi okunmasın (ör. "--dangerously...") ve kabuk karakteri taşımasın: yalnız ad karakterleri.
fn is_valid_model(model: &str) -> bool {
    // C# `model.Length` UTF-16 birim sayısıdır.
    model.encode_utf16().count() <= 80
        && model.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && model.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ':' | '[' | ']'))
}

fn claude_arguments(prompt: &str, permission: VoiceClaudePermission, model: Option<&str>) -> Result<Vec<String>, VoiceCommandException> {
    let mut args: Vec<String> = vec!["-p".to_string()];

    let trimmed = model.map(text::trim);
    if let Some(trimmed) = trimmed {
        if !trimmed.is_empty() && !text::eq_ignore_case(trimmed, DEFAULT_MODEL) {
            if !is_valid_model(trimmed) {
                return Err(VoiceCommandException::with("Invalid model name.", VoiceFailure::InvalidModel, trimmed));
            }
            args.push("--model".to_string());
            args.push(trimmed.to_string());
        }
    }

    match permission {
        VoiceClaudePermission::Auto => {
            args.push("--permission-mode".to_string());
            args.push("auto".to_string());
        }
        VoiceClaudePermission::AcceptEdits => {
            args.push("--permission-mode".to_string());
            args.push("acceptEdits".to_string());
        }
        VoiceClaudePermission::Default => {}
    }

    // "--" istemden önce: seçenekler burada biter.
    args.push("--".to_string());
    args.push(prompt.to_string());
    Ok(args)
}

fn resolve_custom(
    prompt: &str,
    custom_executable: Option<&str>,
    custom_template: Option<&str>,
) -> Result<ResolvedCommand, VoiceCommandException> {
    if text::is_null_or_white_space(custom_executable) {
        return Err(VoiceCommandException::new("Özel harness için yürütülebilir dosya adı boş olamaz."));
    }
    let custom_executable = custom_executable.unwrap_or_default();

    if has_dangerous_shell_character(custom_executable) || custom_template.is_some_and(has_dangerous_shell_character) {
        return Err(VoiceCommandException::new(
            "Komut kalıbında veya yürütülebilir dosya yolunda kabuk metakarakterleri (| & ; > < ` $) bulunamaz.",
        ));
    }

    let exe: String;
    let args: Vec<String>;

    if text::is_null_or_white_space(custom_template) {
        // customExecutable tüm komut satırını içerebilir (örn: 'gh copilot suggest "{prompt}"' veya 'tool.exe')
        let tokens = tokenize(custom_executable);
        if tokens.is_empty() {
            return Err(VoiceCommandException::new("Özel harness komutu boş olamaz."));
        }

        exe = tokens[0].clone();
        let template_tokens: Vec<String> = tokens.into_iter().skip(1).collect();
        args = build_arguments_from_tokens(&template_tokens, prompt);
    } else {
        exe = text::trim(custom_executable).to_string();
        args = VoiceCommandParser::parse_custom_arguments(custom_template, prompt);
    }

    let file_name_only = path_util::get_file_name(&exe);

    if BLOCKED_EXECUTABLES.iter().any(|b| text::eq_ignore_case(b, file_name_only)) {
        return Err(VoiceCommandException::new(format!(
            "Güvenlik kuralı gereğince kabuk yorumlayıcıları ({file_name_only}) harness olarak çalıştırılamaz."
        )));
    }

    if has_dangerous_shell_character(&exe) {
        return Err(VoiceCommandException::new("Yürütülebilir dosya yolunda kabuk metakarakterleri (| & ; > < ` $) bulunamaz."));
    }

    Ok(ResolvedCommand::new(exe, args))
}

fn build_arguments_from_tokens(tokens: &[String], prompt: &str) -> Vec<String> {
    let mut prompt_replaced = false;
    let mut result: Vec<String> = Vec::with_capacity(tokens.len() + 1);

    for token in tokens {
        if text::contains_ignore_case(token, "{prompt}") {
            // Yer tutucuyu tam olarak prompt metniyle değiştir
            result.push(replace_ignore_case(token, "{prompt}", prompt));
            prompt_replaced = true;
        } else {
            result.push(token.clone());
        }
    }

    if !prompt_replaced {
        result.push(prompt.to_string());
    }

    result
}

/// `string.Replace(old, new, StringComparison.OrdinalIgnoreCase)`: soldan sağa, çakışmasız.
fn replace_ignore_case(source: &str, old: &str, new: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let old_len = old.chars().count();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if i + old_len <= chars.len() {
            let window: String = chars[i..i + old_len].iter().collect();
            if text::eq_ignore_case(&window, old) {
                out.push_str(new);
                i += old_len;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn tokenize(input: &str) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    let span: Vec<char> = text::trim(input).chars().collect();
    let mut i = 0;

    while i < span.len() {
        while i < span.len() && span[i].is_whitespace() {
            i += 1;
        }

        if i >= span.len() {
            break;
        }

        let mut in_quotes = false;
        let mut quote_char = '\0';
        let mut current_token = String::new();

        while i < span.len() {
            let c = span[i];
            if !in_quotes && (c == '"' || c == '\'') {
                in_quotes = true;
                quote_char = c;
                i += 1;
                continue;
            }

            if in_quotes && c == quote_char {
                in_quotes = false;
                quote_char = '\0';
                i += 1;
                continue;
            }

            if !in_quotes && c.is_whitespace() {
                break;
            }

            current_token.push(c);
            i += 1;
        }

        if !current_token.is_empty() {
            tokens.push(current_token);
        }
    }

    tokens
}
