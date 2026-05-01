//! Round prompts compiled into the binary via `include_str!`.
//!
//! Per cli-spec.md the CLI must remain functional after install regardless of
//! the workspace `IDL/` directory layout. We therefore embed all five round
//! prompts at compile time and expose them through [`round_prompt`].

const ROUND_1: &str = include_str!("../../../IDL/skills/idl-interview/prompts/round-1.md");
const ROUND_2: &str = include_str!("../../../IDL/skills/idl-interview/prompts/round-2.md");
const ROUND_3: &str = include_str!("../../../IDL/skills/idl-interview/prompts/round-3.md");
const ROUND_4: &str = include_str!("../../../IDL/skills/idl-interview/prompts/round-4.md");
const ROUND_5: &str = include_str!("../../../IDL/skills/idl-interview/prompts/round-5.md");

pub const SKILL_README: &str = include_str!("../../../IDL/skills/idl-interview/SKILL.md");

pub fn round_prompt(n: u32) -> Option<&'static str> {
    match n {
        1 => Some(ROUND_1),
        2 => Some(ROUND_2),
        3 => Some(ROUND_3),
        4 => Some(ROUND_4),
        5 => Some(ROUND_5),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_five_prompts_are_embedded() {
        for n in 1..=5 {
            let p = round_prompt(n).unwrap();
            assert!(p.contains("idl interview"), "round {n} prompt missing header");
        }
        assert!(round_prompt(6).is_none());
    }
}
