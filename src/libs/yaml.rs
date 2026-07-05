use std::fs;

use chrono::{DateTime, Local, NaiveDate, TimeZone};
use yaml_rust::{Yaml, YamlLoader};

pub fn read_yaml(path: &str) -> Yaml {
    let yaml_all = fs::read_to_string(path).expect(format!("failed to load {}", path).as_str());
    let yaml_all = YamlLoader::load_from_str(yaml_all.as_str()).expect("");
    yaml_all.iter().next().unwrap().clone()
}

pub fn as_u16(yaml: &Yaml, key: &str) -> u16 {
    yaml[key]
        .as_i64()
        .expect(format!("no {}", key).as_str())
        .try_into()
        .unwrap()
}

pub fn as_string(yaml: &Yaml, key: &str) -> String {
    yaml[key]
        .as_str()
        .expect(format!("no {}", key).as_str())
        .to_string()
}

pub fn as_bool(yaml: &Yaml, key: &str) -> bool {
    yaml[key].as_bool().expect(format!("no {}", key).as_str())
}

pub fn as_local_datetime(yaml: &Yaml, key: &str) -> DateTime<Local> {
    let str = yaml[key].as_str().expect(format!("no {}", key).as_str());
    let naive_date = NaiveDate::parse_from_str(str, "%Y/%m/%d")
        .expect(format!("failed to parse {} as naive datetime: {}", key, str).as_str());
    let naive_datetime = naive_date.and_hms_opt(0, 0, 0).unwrap();
    Local.from_local_datetime(&naive_datetime).single().unwrap()
}

pub fn as_u16_option(yaml: &Yaml, key: &str) -> Option<u16> {
    yaml[key].as_i64().map(|i| i.try_into().unwrap())
}

pub fn as_f64_option(yaml: &Yaml, key: &str) -> Option<f64> {
    yaml[key].as_f64()
}

pub fn as_string_option(yaml: &Yaml, key: &str) -> Option<String> {
    yaml[key].as_str().map(|s| s.to_string())
}

pub fn as_local_datetime_option(yaml: &Yaml, key: &str) -> Option<DateTime<Local>> {
    yaml[key].as_str().map(|s| {
        let naive_date = NaiveDate::parse_from_str(s, "%Y/%m/%d")
            .expect(format!("failed to parse {} as naive datetime: {}", key, s).as_str());
        let naive_datetime = naive_date.and_hms_opt(0, 0, 0).unwrap();
        Local.from_local_datetime(&naive_datetime).single().unwrap()
    })
}

pub fn as_string_array(yaml: &Yaml, key: &str) -> Vec<String> {
    let mut res = Vec::<String>::new();
    if let Some(v) = yaml[key].as_vec() {
        for s in v {
            res.push(s.as_str().unwrap().to_string());
        }
    }
    res
}

pub fn as_u16_array(yaml: &Yaml, key: &str) -> Vec<u16> {
    let mut res = Vec::<u16>::new();
    if let Some(v) = yaml[key].as_vec() {
        for s in v {
            res.push(s.as_i64().unwrap().try_into().unwrap());
        }
    }
    res
}
