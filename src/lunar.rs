//! Đổi ngày dương sang ngày âm lịch Việt Nam (thuật toán của Hồ Ngọc Đức, múi giờ +7).

use std::f64::consts::PI;

const TZ: f64 = 7.0;

fn jd_from_date(dd: i64, mm: i64, yy: i64) -> i64 {
    let a = (14 - mm) / 12;
    let y = yy + 4800 - a;
    let m = mm + 12 * a - 3;
    let jd = dd + (153 * m + 2) / 5 + 365 * y + y / 4 - y / 100 + y / 400 - 32045;
    if jd < 2299161 {
        dd + (153 * m + 2) / 5 + 365 * y + y / 4 - 32083
    } else {
        jd
    }
}

fn new_moon(k: f64) -> f64 {
    let t = k / 1236.85;
    let (t2, t3) = (t * t, t * t * t);
    let dr = PI / 180.0;
    let mut jd1 = 2415020.75933 + 29.53058868 * k + 0.0001178 * t2 - 0.000000155 * t3;
    jd1 += 0.00033 * ((166.56 + 132.87 * t - 0.009173 * t2) * dr).sin();
    let m = 359.2242 + 29.10535608 * k - 0.0000333 * t2 - 0.00000347 * t3;
    let mpr = 306.0253 + 385.81691806 * k + 0.0107306 * t2 + 0.00001236 * t3;
    let f = 21.2964 + 390.67050646 * k - 0.0016528 * t2 - 0.00000239 * t3;
    let mut c1 = (0.1734 - 0.000393 * t) * (m * dr).sin() + 0.0021 * (2.0 * dr * m).sin();
    c1 = c1 - 0.4068 * (mpr * dr).sin() + 0.0161 * (dr * 2.0 * mpr).sin();
    c1 -= 0.0004 * (dr * 3.0 * mpr).sin();
    c1 = c1 + 0.0104 * (dr * 2.0 * f).sin() - 0.0051 * (dr * (m + mpr)).sin();
    c1 = c1 - 0.0074 * (dr * (m - mpr)).sin() + 0.0004 * (dr * (2.0 * f + m)).sin();
    c1 = c1 - 0.0004 * (dr * (2.0 * f - m)).sin() - 0.0006 * (dr * (2.0 * f + mpr)).sin();
    c1 = c1 + 0.0010 * (dr * (2.0 * f - mpr)).sin() + 0.0005 * (dr * (2.0 * mpr + m)).sin();
    let deltat = if t < -11.0 {
        0.001 + 0.000839 * t + 0.0002261 * t2 - 0.00000845 * t3 - 0.000000081 * t * t3
    } else {
        -0.000278 + 0.000265 * t + 0.000262 * t2
    };
    jd1 + c1 - deltat
}

fn new_moon_day(k: i64) -> i64 {
    (new_moon(k as f64) + 0.5 + TZ / 24.0).floor() as i64
}

fn sun_longitude(jdn: f64) -> f64 {
    let t = (jdn - 2451545.0) / 36525.0;
    let t2 = t * t;
    let dr = PI / 180.0;
    let m = 357.52910 + 35999.05030 * t - 0.0001559 * t2 - 0.00000048 * t * t2;
    let l0 = 280.46645 + 36000.76983 * t + 0.0003032 * t2;
    let mut dl = (1.914600 - 0.004817 * t - 0.000014 * t2) * (dr * m).sin();
    dl = dl + (0.019993 - 0.000101 * t) * (dr * 2.0 * m).sin() + 0.000290 * (dr * 3.0 * m).sin();
    let mut l = (l0 + dl) * dr;
    l -= PI * 2.0 * (l / (PI * 2.0)).floor();
    l
}

fn sun_longitude_sector(day: i64) -> i64 {
    (sun_longitude(day as f64 - 0.5 - TZ / 24.0) / PI * 6.0).floor() as i64
}

fn lunar_month11(yy: i64) -> i64 {
    let off = jd_from_date(31, 12, yy) - 2415021;
    let k = (off as f64 / 29.530588853).floor() as i64;
    let mut nm = new_moon_day(k);
    if sun_longitude_sector(nm) >= 9 {
        nm = new_moon_day(k - 1);
    }
    nm
}

fn leap_month_offset(a11: i64) -> i64 {
    let k = ((a11 as f64 - 2415021.076998695) / 29.530588853 + 0.5).floor() as i64;
    let mut i = 1;
    let mut arc = sun_longitude_sector(new_moon_day(k + i));
    loop {
        let last = arc;
        i += 1;
        arc = sun_longitude_sector(new_moon_day(k + i));
        if !(arc != last && i < 14) {
            break;
        }
    }
    i - 1
}

/// Ngày âm lịch của ngày dương (d, m, y): (ngày, tháng, năm, tháng nhuận?).
pub fn solar_to_lunar(dd: u32, mm: u32, yy: i32) -> (u32, u32, i32, bool) {
    let (dd, mm, yy) = (dd as i64, mm as i64, yy as i64);
    let day_number = jd_from_date(dd, mm, yy);
    let k = ((day_number as f64 - 2415021.076998695) / 29.530588853).floor() as i64;
    let mut month_start = new_moon_day(k + 1);
    if month_start > day_number {
        month_start = new_moon_day(k);
    }
    let mut a11 = lunar_month11(yy);
    let mut b11 = a11;
    let mut lunar_year;
    if a11 >= month_start {
        lunar_year = yy;
        a11 = lunar_month11(yy - 1);
    } else {
        lunar_year = yy + 1;
        b11 = lunar_month11(yy + 1);
    }
    let lunar_day = day_number - month_start + 1;
    let diff = (month_start - a11) / 29;
    let mut leap = false;
    let mut lunar_month = diff + 11;
    if b11 - a11 > 365 {
        let leap_diff = leap_month_offset(a11);
        if diff >= leap_diff {
            lunar_month = diff + 10;
            if diff == leap_diff {
                leap = true;
            }
        }
    }
    if lunar_month > 12 {
        lunar_month -= 12;
    }
    if lunar_month >= 11 && diff < 4 {
        lunar_year -= 1;
    }
    (lunar_day as u32, lunar_month as u32, lunar_year as i32, leap)
}

/// Số ngày từ 1/1 dương lịch tới một ngày trong tháng âm lịch chính (không nhuận).
/// Dùng cho các lễ thuộc tháng 1–10 âm lịch của cùng năm.
pub fn lunar_days_from_january(day: u32, month: u32, year: i32) -> i64 {
    let year = i64::from(year);
    let a11 = lunar_month11(year - 1);
    let b11 = lunar_month11(year);
    let k = ((a11 as f64 - 2415021.076998695) / 29.530588853 + 0.5).floor() as i64;
    let mut offset = i64::from(month) + 1;
    if b11 - a11 > 365 && offset >= leap_month_offset(a11) {
        offset += 1;
    }
    new_moon_day(k + offset) + i64::from(day) - 1 - jd_from_date(1, 1, year)
}

/// Tên năm âm lịch theo can chi, ví dụ 2026 -> "Bính Ngọ".
pub fn can_chi(year: i32) -> String {
    const CAN: [&str; 10] = ["Canh", "Tân", "Nhâm", "Quý", "Giáp", "Ất", "Bính", "Đinh", "Mậu", "Kỷ"];
    const CHI: [&str; 12] = ["Thân", "Dậu", "Tuất", "Hợi", "Tý", "Sửu", "Dần", "Mão", "Thìn", "Tỵ", "Ngọ", "Mùi"];
    format!("{} {}", CAN[year.rem_euclid(10) as usize], CHI[year.rem_euclid(12) as usize])
}
