use std::str::FromStr;

use anyhow::{Result, anyhow};
use whatsapp_rust::Jid;

pub fn parse_chat_or_phone(input: &str) -> Result<Jid> {
    let trimmed = input.trim();
    if trimmed.contains('@') {
        return Jid::from_str(trimmed).map_err(|err| anyhow!("invalid JID {trimmed}: {err}"));
    }

    let digits = normalize_phone(trimmed)?;
    Ok(Jid::pn(digits))
}

pub fn normalize_chat_string(input: &str) -> Result<String> {
    Ok(parse_chat_or_phone(input)?.to_string())
}

pub fn normalize_phone(input: &str) -> Result<String> {
    let digits: String = input.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 7 {
        return Err(anyhow!(
            "phone number must include country code and at least 7 digits"
        ));
    }
    Ok(digits)
}

pub fn chat_kind(jid: &str) -> &'static str {
    match Jid::from_str(jid).map(|j| j.server.to_string()) {
        Ok(server) if server == "g.us" => "group",
        Ok(server) if server == "broadcast" => "broadcast",
        Ok(server) if server == "newsletter" => "newsletter",
        Ok(_) => "dm",
        Err(_) => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_phone_to_person_jid() {
        assert_eq!(
            parse_chat_or_phone("+1 (555) 123-4567")
                .unwrap()
                .to_string(),
            "15551234567@s.whatsapp.net"
        );
    }

    #[test]
    fn accepts_group_jid() {
        assert_eq!(
            parse_chat_or_phone("120363000000000000@g.us")
                .unwrap()
                .to_string(),
            "120363000000000000@g.us"
        );
    }
}
