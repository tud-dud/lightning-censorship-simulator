use log::{debug, warn};
use maxminddb::{geoip2, MaxMindDBError};
use std::net::IpAddr;

static AS_ISP_DB_PATH: &str = "./src/net/geolite2/GeoLite2-ASN_20240116/GeoLite2-ASN.mmdb";

pub struct DbReader {
    reader: maxminddb::Reader<Vec<u8>>,
}

impl DbReader {
    pub fn new() -> Self {
        let reader =
            maxminddb::Reader::open_readfile(AS_ISP_DB_PATH).expect("Error opening database");
        debug!("Succesfully opened AS database.");
        DbReader { reader }
    }

    pub fn lookup_asn(&self, ip: IpAddr) -> u32 {
        let asn: Result<geoip2::Asn, MaxMindDBError> = self.reader.lookup(ip);
        match asn {
            Ok(asn_info) => {
                if let Some(as_num) = asn_info.autonomous_system_number {
                    as_num
                } else {
                    warn!("No ASN entry found for {} in database.", ip);
                    u32::default()
                }
            }
            Err(err) => {
                warn!("ASN lookup for {} failed: {}", ip, err);
                u32::default()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn valid_ip_asn_lookup() {
        let db_reader = DbReader::new();
        let example: IpAddr = FromStr::from_str("93.184.216.34").unwrap();
        let actual = db_reader.lookup_asn(example);
        let expected = 15133;
        assert_eq!(actual, expected);
    }

    #[test]
    fn invalid_ip_asn_lookup() {
        let db_reader = DbReader::new();
        let zero_addr: IpAddr = FromStr::from_str("0.0.0.0").unwrap();
        let actual = db_reader.lookup_asn(zero_addr);
        let expected = 0;
        assert_eq!(actual, expected);
    }
}
