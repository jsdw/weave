
use env_logger::{ Env, Builder };

const LOG: &str = "WEAVE_LOG";
const LOG_STYLE: &str = "WEAVE_LOG_STYLE";

pub fn init() {
    let env = Env::new()
        .filter_or(LOG, "info")
        .write_style(LOG_STYLE);

    Builder::from_env(env)
        .init();
}