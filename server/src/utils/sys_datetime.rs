use chrono::Local;

const YEAR_MONTH_DAY_HOUR_MINUTE_FORMATTER: &str = "%Y-%m-%d %H:%M:00";
const YEAR_MONTH_DAY_HOUR_FORMATTER: &str = "%Y-%m-%d %H";


pub fn now_str_date_and_hour_minute() -> String {
    return now_str(YEAR_MONTH_DAY_HOUR_MINUTE_FORMATTER);
}

pub fn now_str_date_and_hour() -> String {
    return now_str(YEAR_MONTH_DAY_HOUR_FORMATTER);
}

pub fn now_str_data_file_name() -> String {
    return now_str("%Y%m%d%H");
}

pub fn now_str(formatter:&str) -> String {
    return Local::now().format(formatter).to_string();
}