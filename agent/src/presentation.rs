use crate::model::{GlobalSettings, MotdStateSettings, ServerState, SnapshotServer};
use base64::Engine;

fn parse_color(value: &str) -> Option<(u8, u8, u8)> {
    if value.len() != 7 || !value.starts_with('#') {
        return None;
    }
    Some((
        u8::from_str_radix(&value[1..3], 16).ok()?,
        u8::from_str_radix(&value[3..5], 16).ok()?,
        u8::from_str_radix(&value[5..7], 16).ok()?,
    ))
}

fn legacy_hex((r, g, b): (u8, u8, u8)) -> String {
    format!(
        "§x§{:x}§{:x}§{:x}§{:x}§{:x}§{:x}",
        r >> 4,
        r & 15,
        g >> 4,
        g & 15,
        b >> 4,
        b & 15
    )
}

fn normalize_legacy_codes(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '&'
            && let Some(code) = characters.peek().copied()
            && matches!(code.to_ascii_lowercase(), '0'..='9' | 'a'..='f' | 'k'..='o' | 'r')
        {
            output.push('§');
            output.push(code.to_ascii_lowercase());
            characters.next();
            continue;
        }
        output.push(character);
    }
    output
}

pub fn apply_gradient(text: &str, colors: &[String]) -> String {
    let colors: Vec<_> = colors
        .iter()
        .filter_map(|color| parse_color(color))
        .collect();
    if colors.len() < 2 {
        return text.to_owned();
    }
    let visible = text
        .chars()
        .filter(|character| *character != '\n' && !character.is_whitespace())
        .count()
        .max(1);
    let mut output = String::with_capacity(text.len() * 8);
    let mut index = 0usize;
    let chars: Vec<char> = text.chars().collect();
    let mut position = 0usize;
    while position < chars.len() {
        let character = chars[position];
        if character == '§' && position + 1 < chars.len() {
            output.push(character);
            output.push(chars[position + 1]);
            position += 2;
            continue;
        }
        if character == '\n' || character.is_whitespace() {
            output.push(character);
            position += 1;
            continue;
        }

        let scaled =
            index as f32 / visible.saturating_sub(1).max(1) as f32 * (colors.len() - 1) as f32;
        let segment = (scaled.floor() as usize).min(colors.len() - 2);
        let amount = scaled - segment as f32;
        let start = colors[segment];
        let end = colors[segment + 1];
        let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * amount).round() as u8;
        output.push_str(&legacy_hex((
            mix(start.0, end.0),
            mix(start.1, end.1),
            mix(start.2, end.2),
        )));
        output.push(character);
        index += 1;
        position += 1;
    }
    output
}

fn placeholders(value: &str, server: &SnapshotServer, state: ServerState) -> String {
    value
        .replace("{server_name}", &server.server_name)
        .replace("{node_name}", &server.node_name)
        .replace("{state}", state.key())
}

fn swap_lines(value: String) -> String {
    let mut lines = value.splitn(3, '\n');
    match (lines.next(), lines.next()) {
        (Some(first), Some(second)) => match lines.next() {
            Some(rest) => format!("{second}\n{first}\n{rest}"),
            None => format!("{second}\n{first}"),
        },
        _ => value,
    }
}

pub fn selected_state(settings: &GlobalSettings, state: ServerState) -> Option<&MotdStateSettings> {
    settings
        .states
        .get(state.key())
        .or_else(|| settings.states.get("offline"))
}

pub fn status_json(
    settings: &GlobalSettings,
    server: &SnapshotServer,
    state: ServerState,
    client_protocol: i32,
) -> anyhow::Result<serde_json::Value> {
    let config = selected_state(settings, state)
        .ok_or_else(|| anyhow::anyhow!("missing state presentation"))?;
    let index = (chrono::Utc::now().timestamp().unsigned_abs()
        / settings.rotation_interval_seconds.max(1)) as usize
        % config.descriptions.len().max(1);
    let mut description = normalize_legacy_codes(&placeholders(
        config
            .descriptions
            .get(index)
            .map(String::as_str)
            .unwrap_or_default(),
        server,
        state,
    ));
    if settings.swap_lines {
        description = swap_lines(description);
    }
    if settings.gradient_enabled {
        description = apply_gradient(&description, &settings.gradient_colors);
    }

    let icon = settings.favicon_base64.clone().unwrap_or_else(|| {
        base64::engine::general_purpose::STANDARD
            .encode(include_bytes!("../assets/server-icon.png"))
    });
    Ok(serde_json::json!({
        "version": {
            "name": config.version,
            "protocol": config.protocol.unwrap_or(client_protocol),
        },
        "players": {
            "online": config.online_players,
            "max": config.max_players,
            "sample": [],
        },
        "description": { "text": description },
        "favicon": format!("data:image/png;base64,{icon}"),
        "enforcesSecureChat": false,
    }))
}

pub fn kick_message(
    settings: &GlobalSettings,
    server: &SnapshotServer,
    state: ServerState,
) -> anyhow::Result<String> {
    let config = selected_state(settings, state)
        .ok_or_else(|| anyhow::anyhow!("missing state presentation"))?;
    Ok(normalize_legacy_codes(&placeholders(
        &config.kick_message,
        server,
        state,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gradient_keeps_text_and_adds_hex_codes() {
        let result = apply_gradient("Hi", &["#000000".into(), "#ffffff".into()]);
        assert!(result.contains('H'));
        assert!(result.contains('i'));
        assert!(result.starts_with("§x"));
    }

    #[test]
    fn line_swap_only_changes_first_two_lines() {
        assert_eq!(swap_lines("one\ntwo\nthree".into()), "two\none\nthree");
    }

    #[test]
    fn ampersand_color_codes_are_normalized() {
        assert_eq!(
            normalize_legacy_codes("&aReady && waiting"),
            "§aReady && waiting"
        );
    }
}
