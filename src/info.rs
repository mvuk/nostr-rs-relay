//! Relay metadata using NIP-11
/// Relay Info
use crate::config::Settings;
use serde::{Deserialize, Serialize};

pub const CARGO_PKG_VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");
pub const UNIT: &str = "msats";

/// Limitations of the relay as specified in NIP-111
/// (This nip isn't finalized so may change)
#[derive(Debug, Serialize, Deserialize)]
#[allow(unused)]
pub struct Limitation {
    #[serde(skip_serializing_if = "Option::is_none")]
    payment_required: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    restricted_writes: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(unused)]
pub struct Fees {
    #[serde(skip_serializing_if = "Option::is_none")]
    admission: Option<Vec<Fee>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    publication: Option<Vec<Fee>>,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(unused)]
pub struct Fee {
    amount: u64,
    unit: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(unused)]
pub struct RelayInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pubkey: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supported_nips: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub software: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limitation: Option<Limitation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payments_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fees: Option<Fees>,
}

/// Build an HTTP(S) URL for a page served by the relay itself, from the
/// advertised websocket URL. Only the scheme is changed (`ws` to `http`,
/// `wss` to `https`); `path` is resolved relative to the relay URL.
fn relay_http_url(relay_url: &str, path: &str) -> Option<String> {
    let mut url = url::Url::parse(relay_url).ok()?;
    let scheme = match url.scheme() {
        "ws" => "http",
        "wss" => "https",
        "http" | "https" => url.scheme(),
        _ => return None,
    }
    .to_owned();
    url.set_scheme(&scheme).ok()?;
    url.join(path).ok().map(String::from)
}

/// Convert an Info configuration into public Relay Info
impl From<Settings> for RelayInfo {
    fn from(c: Settings) -> Self {
        let mut supported_nips = vec![1, 2, 9, 11, 12, 15, 16, 20, 22, 33, 40];

        if c.authorization.nip42_auth {
            supported_nips.push(42);
            supported_nips.sort();
        }

        let i = c.info;
        let p = c.pay_to_relay;

        let limitations = Limitation {
            payment_required: Some(p.enabled),
            restricted_writes: Some(
                p.enabled
                    || c.verified_users.is_enabled()
                    || c.authorization.pubkey_whitelist.is_some()
                    || c.grpc.restricts_write,
            ),
        };

        let (payments_url, fees) = if p.enabled {
            let admission_fee = if p.admission_cost > 0 {
                Some(vec![Fee {
                    amount: p.admission_cost * 1000,
                    unit: UNIT.to_string(),
                }])
            } else {
                None
            };

            let post_fee = if p.cost_per_event > 0 {
                Some(vec![Fee {
                    amount: p.cost_per_event * 1000,
                    unit: UNIT.to_string(),
                }])
            } else {
                None
            };

            let fees = Fees {
                admission: admission_fee,
                publication: post_fee,
            };

            let payments_url = i
                .relay_url
                .as_deref()
                .and_then(|url| relay_http_url(url, "join"));
            (payments_url, Some(fees))
        } else {
            (None, None)
        };

        RelayInfo {
            id: i.relay_url,
            name: i.name,
            description: i.description,
            pubkey: i.pubkey,
            contact: i.contact,
            supported_nips: Some(supported_nips),
            software: Some("https://git.sr.ht/~gheartsfield/nostr-rs-relay".to_owned()),
            version: CARGO_PKG_VERSION.map(std::borrow::ToOwned::to_owned),
            limitation: Some(limitations),
            payments_url,
            fees,
            icon: i.relay_icon,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paid_settings() -> Settings {
        let mut settings = Settings::default();
        settings.info.relay_url = Some("wss://relay.example.com/".to_owned());
        settings.pay_to_relay.enabled = true;
        settings.pay_to_relay.admission_cost = 4200;
        settings.pay_to_relay.cost_per_event = 1;
        settings
    }

    #[test]
    fn payments_url_uses_nip11_field_name() {
        let info = RelayInfo::from(paid_settings());
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["payments_url"], "https://relay.example.com/join");
        assert!(json.get("payment_url").is_none());
    }

    #[test]
    fn payments_url_only_rewrites_scheme() {
        assert_eq!(
            relay_http_url("wss://ws.example.com/", "join").as_deref(),
            Some("https://ws.example.com/join")
        );
        assert_eq!(
            relay_http_url("ws://news.example.com", "join").as_deref(),
            Some("http://news.example.com/join")
        );
        assert_eq!(
            relay_http_url("wss://example.com/relay/", "join").as_deref(),
            Some("https://example.com/relay/join")
        );
        assert_eq!(relay_http_url("not a url", "join"), None);
    }

    #[test]
    fn fees_from_pay_to_relay_in_msats() {
        let info = RelayInfo::from(paid_settings());
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["fees"]["admission"][0]["amount"], 4_200_000);
        assert_eq!(json["fees"]["admission"][0]["unit"], "msats");
        assert_eq!(json["fees"]["publication"][0]["amount"], 1000);
        assert_eq!(json["limitation"]["payment_required"], true);
    }

    #[test]
    fn no_payment_fields_when_pay_to_relay_disabled() {
        let mut settings = paid_settings();
        settings.pay_to_relay.enabled = false;
        let json = serde_json::to_value(RelayInfo::from(settings)).unwrap();
        assert!(json.get("payments_url").is_none());
        assert!(json.get("fees").is_none());
        assert_eq!(json["limitation"]["payment_required"], false);
    }
}
