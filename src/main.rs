extern crate rustc_serialize;
use std::env;
use std::fs;
use std::sync::{Arc, Mutex};
use regex::Regex;
use rustc_serialize::json;
use std::io::{self, Write};
use reqwest::header;
use std::time;
use tokio::task;

#[derive(Debug, RustcDecodable, RustcEncodable)]
struct Info {
    pub current_name: String,
    pub rate: f64
}

impl Info {
    pub fn new(current_name: &str, rate: f64) -> Self {
        Info {
            current_name: String::from(current_name),
            rate: rate
        }
    }
}


fn get_cached_dir() -> String {
    // $XDG_CACHE_HOME defines the base directory relative to which user specific non-essential
    // data files should be stored. If $XDG_CACHE_HOME is either not set or empty, a default equal
    // to $HOME/.cache should be used.
    let xdg_cache_home = "XDG_CACHE_HOME";
    let home = "HOME";
    let dir = match env::var_os(xdg_cache_home) {
        Some(val) => val.into_string().unwrap(),
        None => {
            match env::var_os(home) {
                Some(val) => {
                    let mut home = val.into_string().unwrap();
                    home.push_str("/.cache");
                    home
                },
                None => "".to_owned()
            }
        }
    };
    assert!(dir != "");
    dir
}

fn read_cached_exchange_rate(exchange_rate: Arc<Mutex<Vec<Info>>>) {
    let cached_dir = get_cached_dir();
    let cached_file = cached_dir + "/exchange_rate.json";
    let info = fs::read_to_string(cached_file);
    if info.is_err() {
        return;
    }
    let info = info.unwrap();
    let infos = json::decode(&info);
    if infos.is_err() {
        return;
    }
    let infos = infos.unwrap();
    let mut data = exchange_rate.lock().unwrap();
    if (*data).len() == 0 {
        *data = infos;
    }
}

fn write_cached_exchange_rate(exchange_rate: Arc<Mutex<Vec<Info>>>) {
    let cached_dir = get_cached_dir();
    let cached_file = cached_dir + "/exchange_rate.json";
    let data = exchange_rate.lock().unwrap();
    let encoded = json::encode(&*data).unwrap();
    if fs::write(cached_file, encoded).is_err() {
        println!("cache the data failed");
    }
}

async fn fetch_info_from_web(exchange_rate: Arc<Mutex<Vec<Info>>>) -> Result<(), reqwest::Error> {
    let current_list = vec!["USD", "HKD", "JPY", "ARS", "TRY", "RUB", "EUR", "GBP", "TWD", "KRW", "AUD"];
    let re_str_list: Vec<String> = current_list.iter().map(
        |current_name| format!(r#"(\d+\.\d+)</a> <a href="/{}__huobiduihuan/" title=".*?">(.*?)<"#, current_name)
    ).collect();
    let mut headers = header::HeaderMap::new();
    headers.insert(
        "User_Agent", 
        header::HeaderValue::from_str(
            "Mozilla/5.0 (X11; Linux x86_64; rv:74.0) Gecko/20100101 Firefox/74.0")
        .unwrap());
    let client = reqwest::Client::builder()
        .timeout(time::Duration::from_secs(10))
        .default_headers(headers)
        .build()?;
    let resp_html_str: String = client.get("https://huobiduihuan.bmcx.com/").send().await?.text().await?;
    let mut new_exchange_rate: Vec<Info> = Vec::new();
    for re_str in re_str_list {
        let re = Regex::new(&re_str).unwrap();
        let caps = re.captures(&resp_html_str).unwrap();
        let rate_str = caps.get(1).unwrap().as_str();
        let rate: f64 = rate_str.parse().unwrap();
        let current_name = caps.get(2).unwrap().as_str();
        new_exchange_rate.push(Info::new(current_name, rate));
    }
    let mut data = exchange_rate.lock().unwrap();
    *data = new_exchange_rate;
    Ok(())
}

fn calc(money: f64, exchange_rate: Arc<Mutex<Vec<Info>>>) {
    let data = exchange_rate.lock().unwrap();
    if (*data).is_empty() {
        return;
    }
    for info in &*data {
        let exchange_money = money / info.rate;
        let exchange_money2 = money * info.rate;
        println!("{} {} = {} 人民币 (CNY) ---- {} 人民币 (CNY) = {} {}", money, info.current_name, exchange_money, money, exchange_money2, info.current_name);
    }
    print!("\n\n");
}

async fn get_input_calc(exchange_rate: Arc<Mutex<Vec<Info>>>) -> f64 {
    read_cached_exchange_rate(exchange_rate.clone());
    print!("Money: ");
    io::stdout().flush().expect("flush failed!");
    let buf = task::spawn(async{
        let mut buf = String::new();
        io::stdin().read_line(&mut buf).expect("read input failed");
        buf
    }).await.unwrap();
    let money: f64 = buf.trim().parse().expect("please enter a number");
    calc(money, exchange_rate.clone());
    money
}

#[tokio::main]
async fn main() {
    let exchange_rate: Arc<Mutex<Vec<Info>>> = Arc::new(Mutex::new(Vec::new()));
    let input_and_calc = get_input_calc(exchange_rate.clone());
    let fetch = fetch_info_from_web(exchange_rate.clone());
    let (money, _) = tokio::join!(input_and_calc, fetch);
    calc(money, exchange_rate.clone());
    write_cached_exchange_rate(exchange_rate.clone());
}
