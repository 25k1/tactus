use super::*;

impl AccountService for YandexService {
    fn account(&self) -> Result<Account> {
        Ok(parse::account(&net::block_on(self.get("account/status", &[]))?))
    }

    fn account_settings(&self) -> Result<AccountSettings> {
        Ok(parse::account_settings(&net::block_on(self.get("account/settings", &[]))?))
    }

    fn set_account_option(&self, option: AccountOption, enabled: bool) -> Result<()> {
        net::block_on(self.post("account/settings", &[(option.key(), option.encode(enabled))]))?;
        Ok(())
    }

    fn consume_promo_code(&self, code: &str) -> Result<String> {
        let options = ConsumePromoCodeOptions::new(code, net::language());
        let result = net::block_on(self.client.consume_promo_code(&options))?;
        Ok(if result.status_desc.is_empty() { result.status } else { result.status_desc })
    }
}
