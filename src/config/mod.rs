pub mod parse;
pub mod settings;
pub mod types;

#[cfg(test)]
mod test {
    use crate::config::parse::jsm_parse;

    #[test]
    fn parse_all_settings() -> anyhow::Result<()> {
        let settings_str = include_str!("all-settings-example");
        let _ = jsm_parse(settings_str)?;
        Ok(())
    }
}
