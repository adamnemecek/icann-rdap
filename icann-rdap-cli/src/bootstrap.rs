use core::panic;
use std::{
    fs::{self, File},
    io::{BufRead, BufReader},
};

use icann_rdap_client::query::qtype::QueryType;
use icann_rdap_common::{
    cache::HttpData,
    iana::{iana_request, IanaRegistry, IanaRegistryType},
};
use ipnet::{Ipv4Net, Ipv6Net};
use prefix_trie::PrefixMap;
use reqwest::Client;
use simplelog::debug;

use crate::{dirs::bootstrap_cache_path, error::CliError};

pub(crate) enum BootstrapType {
    None,
    Url(String),
    Tag(String),
}

pub(crate) async fn get_base_url(
    bootstrap_type: &BootstrapType,
    client: &Client,
    query_type: &QueryType,
) -> Result<String, CliError> {
    if let QueryType::Url(url) = query_type {
        // this is ultimately ignored without this logic a bootstrap not found error is thrown
        // which is wrong for URL queries.
        return Ok(url.to_owned());
    }

    match bootstrap_type {
        BootstrapType::None => qtype_to_bootstrap_url(client, query_type).await,
        BootstrapType::Url(url) => Ok(url.to_owned()),
        BootstrapType::Tag(tag) => {
            let iana = get_iana_registry(IanaRegistryType::RdapObjectTags, client).await?;
            let urls = get_tag_bootstrap_urls(iana, tag)?;
            Ok(get_preferred_url(urls)?)
        }
    }
}

pub(crate) async fn qtype_to_bootstrap_url(
    client: &Client,
    query_type: &QueryType,
) -> Result<String, CliError> {
    match query_type {
        QueryType::IpV4Addr(_) | QueryType::IpV4Cidr(_) => {
            let iana = get_iana_registry(IanaRegistryType::RdapBootstrapIpv4, client).await?;
            let urls = get_ipv4_bootstrap_urls(iana, query_type)?;
            Ok(get_preferred_url(urls)?)
        }
        QueryType::IpV6Addr(_) | QueryType::IpV6Cidr(_) => {
            let iana = get_iana_registry(IanaRegistryType::RdapBootstrapIpv6, client).await?;
            let urls = get_ipv6_bootstrap_urls(iana, query_type)?;
            Ok(get_preferred_url(urls)?)
        }
        QueryType::AsNumber(_) => {
            let iana = get_iana_registry(IanaRegistryType::RdapBootstrapAsn, client).await?;
            let urls = get_asn_bootstrap_urls(iana, query_type)?;
            Ok(get_preferred_url(urls)?)
        }
        QueryType::Domain(_) => {
            let iana = get_iana_registry(IanaRegistryType::RdapBootstrapDns, client).await?;
            let urls = get_domain_bootstrap_urls(iana, query_type)?;
            Ok(get_preferred_url(urls)?)
        }
        QueryType::Entity(_) => {
            let iana = get_iana_registry(IanaRegistryType::RdapObjectTags, client).await?;
            let urls = get_entity_handle_bootstrap_urls(iana, query_type)?;
            Ok(get_preferred_url(urls)?)
        }
        QueryType::Nameserver(_) => {
            let iana = get_iana_registry(IanaRegistryType::RdapBootstrapDns, client).await?;
            let urls = get_domain_bootstrap_urls(iana, query_type)?;
            Ok(get_preferred_url(urls)?)
        }
        _ => Err(CliError::BootstrapNotFound),
    }
}

/// Prefer HTTPS urls.
fn get_preferred_url(urls: Vec<String>) -> Result<String, CliError> {
    if urls.is_empty() {
        Err(CliError::InvalidBootstrap)
    } else {
        let url = urls
            .iter()
            .find(|s| s.starts_with("https://"))
            .unwrap_or_else(|| urls.first().unwrap());
        Ok(url.to_owned())
    }
}

/// Gets the bootstrap url from IANA. Requirements are that it must be the longest match.
fn get_domain_bootstrap_urls(
    iana: IanaRegistry,
    query_type: &QueryType,
) -> Result<Vec<String>, CliError> {
    let domain_name = match query_type {
        QueryType::Domain(domain) => domain,
        QueryType::Nameserver(ns) => ns,
        _ => panic!("invalid domain query type"),
    };
    let mut longest_match: Option<(usize, Vec<String>)> = None;
    let IanaRegistry::RdapBootstrapRegistry(bootstrap) = iana;
    for service in bootstrap.services {
        let tlds = service.first().ok_or(CliError::InvalidBootstrap)?;
        for tld in tlds {
            if domain_name.ends_with(tld) {
                let urls = service.last().ok_or(CliError::InvalidBootstrap)?;
                let longest = longest_match.get_or_insert_with(|| (tld.len(), urls.to_owned()));
                if longest.0 < tld.len() {
                    *longest = (tld.len(), urls.to_owned());
                }
            }
        }
    }
    let longest = longest_match.ok_or(CliError::BootstrapNotFound)?;
    Ok(longest.1)
}

fn get_asn_bootstrap_urls(
    iana: IanaRegistry,
    query_type: &QueryType,
) -> Result<Vec<String>, CliError> {
    let QueryType::AsNumber(asn) = query_type else {panic!("invalid query type")};
    let autnum = asn
        .trim_start_matches(|c| -> bool { matches!(c, 'a' | 'A' | 's' | 'S') })
        .parse::<u32>()
        .map_err(|_| CliError::InvalidBootstrap)?;
    let IanaRegistry::RdapBootstrapRegistry(bootstrap) = iana;
    for service in bootstrap.services {
        let as_range = service
            .first()
            .ok_or(CliError::InvalidBootstrap)?
            .first()
            .ok_or(CliError::BootstrapNotFound)?;
        let as_split = as_range.split('-').collect::<Vec<&str>>();
        let start_as = as_split
            .first()
            .ok_or(CliError::InvalidBootstrap)?
            .parse::<u32>()
            .map_err(|_| CliError::InvalidBootstrap)?;
        let end_as = as_split
            .last()
            .ok_or(CliError::InvalidBootstrap)?
            .parse::<u32>()
            .map_err(|_| CliError::InvalidBootstrap)?;
        if start_as <= autnum && end_as >= autnum {
            let urls = service.last().ok_or(CliError::InvalidBootstrap)?;
            return Ok(urls.to_owned());
        }
    }
    Err(CliError::BootstrapNotFound)
}

fn get_ipv4_bootstrap_urls(
    iana: IanaRegistry,
    query_type: &QueryType,
) -> Result<Vec<String>, CliError> {
    let ip = match query_type {
        QueryType::IpV4Addr(addr) => format!("{addr}/32"),
        QueryType::IpV4Cidr(cidr) => cidr.to_owned(),
        _ => panic!("non ip query for ip bootstrap"),
    };
    let mut pm: PrefixMap<Ipv4Net, Vec<String>> = PrefixMap::new();
    let IanaRegistry::RdapBootstrapRegistry(bootstrap) = iana;
    for service in bootstrap.services {
        let urls = service.last().ok_or(CliError::InvalidBootstrap)?;
        for cidr in service.first().ok_or(CliError::InvalidBootstrap)? {
            pm.insert(
                cidr.parse().map_err(|_| CliError::InvalidBootstrap)?,
                urls.clone(),
            );
        }
    }
    let net = pm
        .get_lpm(&ip.parse().map_err(|_| CliError::InvalidBootstrap)?)
        .ok_or(CliError::BootstrapNotFound)?;
    Ok(net.1.to_owned())
}

fn get_ipv6_bootstrap_urls(
    iana: IanaRegistry,
    query_type: &QueryType,
) -> Result<Vec<String>, CliError> {
    let ip = match query_type {
        QueryType::IpV6Addr(addr) => format!("{addr}/128"),
        QueryType::IpV6Cidr(cidr) => cidr.to_owned(),
        _ => panic!("non ip query for ip bootstrap"),
    };
    let mut pm: PrefixMap<Ipv6Net, Vec<String>> = PrefixMap::new();
    let IanaRegistry::RdapBootstrapRegistry(bootstrap) = iana;
    for service in bootstrap.services {
        let urls = service.last().ok_or(CliError::InvalidBootstrap)?;
        for cidr in service.first().ok_or(CliError::InvalidBootstrap)? {
            pm.insert(
                cidr.parse().map_err(|_| CliError::InvalidBootstrap)?,
                urls.clone(),
            );
        }
    }
    let net = pm
        .get_lpm(&ip.parse().map_err(|_| CliError::InvalidBootstrap)?)
        .ok_or(CliError::BootstrapNotFound)?;
    Ok(net.1.to_owned())
}

fn get_entity_handle_bootstrap_urls(
    iana: IanaRegistry,
    query_type: &QueryType,
) -> Result<Vec<String>, CliError> {
    let QueryType::Entity(handle) = query_type else {panic!("non entity handle for bootstrap")};
    let handle_split = handle.rsplit_once('-').ok_or(CliError::BootstrapNotFound)?;
    get_tag_bootstrap_urls(iana, handle_split.1)
}

fn get_tag_bootstrap_urls(iana: IanaRegistry, tag: &str) -> Result<Vec<String>, CliError> {
    let IanaRegistry::RdapBootstrapRegistry(bootstrap) = iana;
    for service in bootstrap.services {
        let object_tag = service
            .get(1)
            .ok_or(CliError::InvalidBootstrap)?
            .first()
            .ok_or(CliError::InvalidBootstrap)?;
        if object_tag.to_ascii_uppercase() == tag.to_ascii_uppercase() {
            let urls = service.last().ok_or(CliError::InvalidBootstrap)?;
            return Ok(urls.to_owned());
        }
    }
    Err(CliError::BootstrapNotFound)
}

async fn get_iana_registry(
    reg_type: IanaRegistryType,
    client: &Client,
) -> Result<IanaRegistry, CliError> {
    let path = bootstrap_cache_path().join(reg_type.file_name());
    if path.exists() {
        let input = File::open(&path)?;
        let buf = BufReader::new(input);
        let mut lines = Vec::new();
        for line in buf.lines() {
            lines.push(line?);
        }
        let cache_data = HttpData::from_lines(&lines)?;
        if !cache_data.0.is_expired(604800i64) {
            debug!("Getting bootstrap from {}", reg_type.file_name());
            let iana: IanaRegistry = serde_json::from_str(&cache_data.1.join(""))?;
            return Ok(iana);
        }
    }
    debug!("Getting IANA bootstrap from {}", reg_type.url());
    let iana = iana_request(reg_type, client).await?;
    let data = serde_json::to_string_pretty(&iana.registry)?;
    let cache_contents = iana.http_data.to_lines(&data)?;
    fs::write(path, cache_contents)?;
    Ok(iana.registry)
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use icann_rdap_client::query::qtype::QueryType;
    use icann_rdap_common::iana::IanaRegistry;

    use crate::bootstrap::{
        get_asn_bootstrap_urls, get_ipv4_bootstrap_urls, get_ipv6_bootstrap_urls,
        get_tag_bootstrap_urls,
    };

    use super::{get_domain_bootstrap_urls, get_preferred_url};

    #[test]
    fn GIVEN_one_url_WHEN_preferred_urls_THEN_that_is_the_one() {
        // GIVEN
        let urls = vec!["http://foo.example".to_string()];

        // WHEN
        let actual = get_preferred_url(urls).expect("cannot get preferred url");

        // THEN
        assert_eq!(actual, "http://foo.example");
    }

    #[test]
    fn GIVEN_one_http_and_https_url_WHEN_preferred_urls_THEN_return_https() {
        // GIVEN
        let urls = vec![
            "http://foo.example".to_string(),
            "https://foo.example".to_string(),
        ];

        // WHEN
        let actual = get_preferred_url(urls).expect("cannot get preferred url");

        // THEN
        assert_eq!(actual, "https://foo.example");
    }

    #[test]
    fn GIVEN_domain_bootstrap_with_matching_WHEN_find_THEN_url_matches() {
        // GIVEN
        let bootstrap = r#"
            {
                "version": "1.0",
                "publication": "2024-01-07T10:11:12Z",
                "description": "Some text",
                "services": [
                  [
                    ["net", "com"],
                    [
                      "https://registry.example.com/myrdap/"
                    ]
                  ],
                  [
                    ["org", "mytld"],
                    [
                      "https://example.org/"
                    ]
                  ]
                ]
            }
        "#;
        let iana =
            serde_json::from_str::<IanaRegistry>(bootstrap).expect("cannot parse domain bootstrap");

        // WHEN
        let actual = get_domain_bootstrap_urls(iana, &QueryType::Domain("foo.org".to_string()));

        // THEN
        assert_eq!(
            actual.expect("no vec").first().expect("vec is empty"),
            "https://example.org/"
        );
    }

    #[test]
    fn GIVEN_domain_bootstrap_with_two_matching_WHEN_find_THEN_return_longest_match() {
        // GIVEN
        let bootstrap = r#"
            {
                "version": "1.0",
                "publication": "2024-01-07T10:11:12Z",
                "description": "Some text",
                "services": [
                  [
                    ["co.uk"],
                    [
                      "https://registry.co.uk/"
                    ]
                  ],
                  [
                    ["uk"],
                    [
                      "https://registry.uk/"
                    ]
                  ]
                ]
            }
        "#;
        let iana =
            serde_json::from_str::<IanaRegistry>(bootstrap).expect("cannot parse domain bootstrap");

        // WHEN
        let actual = get_domain_bootstrap_urls(iana, &QueryType::Domain("foo.co.uk".to_string()));

        // THEN
        assert_eq!(
            actual.expect("no vec").first().expect("vec is empty"),
            "https://registry.co.uk/"
        );
    }

    #[test]
    fn GIVEN_autnum_bootstrap_with_match_WHEN_find_with_string_THEN_return_match() {
        // GIVEN
        let bootstrap = r#"
            {
                "version": "1.0",
                "publication": "2024-01-07T10:11:12Z",
                "description": "RDAP Bootstrap file for example registries.",
                "services": [
                  [
                    ["64496-64496"],
                    [
                      "https://rir3.example.com/myrdap/"
                    ]
                  ],
                  [
                    ["64497-64510", "65536-65551"],
                    [
                      "https://example.org/"
                    ]
                  ],
                  [
                    ["64512-65534"],
                    [
                      "http://example.net/rdaprir2/",
                      "https://example.net/rdaprir2/"
                    ]
                  ]
                ]
            }
        "#;
        let iana =
            serde_json::from_str::<IanaRegistry>(bootstrap).expect("cannot parse autnum bootstrap");

        // WHEN
        let actual = get_asn_bootstrap_urls(iana, &QueryType::AsNumber("as64498".to_string()));

        // THEN
        assert_eq!(
            actual.expect("no vec").first().expect("vec is empty"),
            "https://example.org/"
        );
    }

    #[test]
    fn GIVEN_autnum_bootstrap_with_match_WHEN_find_with_number_THEN_return_match() {
        // GIVEN
        let bootstrap = r#"
            {
                "version": "1.0",
                "publication": "2024-01-07T10:11:12Z",
                "description": "RDAP Bootstrap file for example registries.",
                "services": [
                  [
                    ["64496-64496"],
                    [
                      "https://rir3.example.com/myrdap/"
                    ]
                  ],
                  [
                    ["64497-64510", "65536-65551"],
                    [
                      "https://example.org/"
                    ]
                  ],
                  [
                    ["64512-65534"],
                    [
                      "http://example.net/rdaprir2/",
                      "https://example.net/rdaprir2/"
                    ]
                  ]
                ]
            }
        "#;
        let iana =
            serde_json::from_str::<IanaRegistry>(bootstrap).expect("cannot parse autnum bootstrap");

        // WHEN
        let actual = get_asn_bootstrap_urls(iana, &QueryType::AsNumber("64498".to_string()));

        // THEN
        assert_eq!(
            actual.expect("no vec").first().expect("vec is empty"),
            "https://example.org/"
        );
    }

    #[test]
    fn GIVEN_ipv4_bootstrap_with_match_WHEN_find_with_ip_address_THEN_return_match() {
        // GIVEN
        let bootstrap = r#"
            {
                "version": "1.0",
                "publication": "2024-01-07T10:11:12Z",
                "description": "RDAP Bootstrap file for example registries.",
                "services": [
                  [
                    ["198.51.100.0/24", "192.0.0.0/8"],
                    [
                      "https://rir1.example.com/myrdap/"
                    ]
                  ],
                  [
                    ["203.0.113.0/24", "192.0.2.0/24"],
                    [
                      "https://example.org/"
                    ]
                  ],
                  [
                    ["203.0.113.0/28"],
                    [
                      "https://example.net/rdaprir2/",
                      "http://example.net/rdaprir2/"
                    ]
                  ]
                ]
            }
        "#;
        let iana =
            serde_json::from_str::<IanaRegistry>(bootstrap).expect("cannot parse ipv4 bootstrap");

        // WHEN
        let actual =
            get_ipv4_bootstrap_urls(iana, &QueryType::IpV4Addr("198.51.100.1".to_string()));

        // THEN
        assert_eq!(
            actual.expect("no vec").first().expect("vec is empty"),
            "https://rir1.example.com/myrdap/"
        );
    }

    #[test]
    fn GIVEN_ipv4_bootstrap_with_match_WHEN_find_with_cidr_THEN_return_match() {
        // GIVEN
        let bootstrap = r#"
            {
                "version": "1.0",
                "publication": "2024-01-07T10:11:12Z",
                "description": "RDAP Bootstrap file for example registries.",
                "services": [
                  [
                    ["198.51.100.0/24", "192.0.0.0/8"],
                    [
                      "https://rir1.example.com/myrdap/"
                    ]
                  ],
                  [
                    ["203.0.113.0/24", "192.0.2.0/24"],
                    [
                      "https://example.org/"
                    ]
                  ],
                  [
                    ["203.0.113.0/28"],
                    [
                      "https://example.net/rdaprir2/",
                      "http://example.net/rdaprir2/"
                    ]
                  ]
                ]
            }
        "#;
        let iana =
            serde_json::from_str::<IanaRegistry>(bootstrap).expect("cannot parse ipv4 bootstrap");

        // WHEN
        let actual =
            get_ipv4_bootstrap_urls(iana, &QueryType::IpV4Cidr("203.0.113.0/24".to_string()));

        // THEN
        assert_eq!(
            actual.expect("no vec").first().expect("vec is empty"),
            "https://example.org/"
        );
    }

    #[test]
    fn GIVEN_ipv6_bootstrap_with_match_WHEN_find_with_ip_address_THEN_return_match() {
        // GIVEN
        let bootstrap = r#"
            {
                "version": "1.0",
                "publication": "2024-01-07T10:11:12Z",
                "description": "RDAP Bootstrap file for example registries.",
                "services": [
                  [
                    ["2001:db8::/34"],
                    [
                      "https://rir2.example.com/myrdap/"
                    ]
                  ],
                  [
                    ["2001:db8:4000::/36", "2001:db8:ffff::/48"],
                    [
                      "https://example.org/"
                    ]
                  ],
                  [
                    ["2001:db8:1000::/36"],
                    [
                      "https://example.net/rdaprir2/",
                      "http://example.net/rdaprir2/"
                    ]
                  ]
                ]
            }
        "#;
        let iana =
            serde_json::from_str::<IanaRegistry>(bootstrap).expect("cannot parse ipv6 bootstrap");

        // WHEN
        let actual = get_ipv6_bootstrap_urls(iana, &QueryType::IpV6Addr("2001:db8::1".to_string()));

        // THEN
        assert_eq!(
            actual.expect("no vec").first().expect("vec is empty"),
            "https://rir2.example.com/myrdap/"
        );
    }

    #[test]
    fn GIVEN_ipv6_bootstrap_with_match_WHEN_find_with_ip_cidr_THEN_return_match() {
        // GIVEN
        let bootstrap = r#"
            {
                "version": "1.0",
                "publication": "2024-01-07T10:11:12Z",
                "description": "RDAP Bootstrap file for example registries.",
                "services": [
                  [
                    ["2001:db8::/34"],
                    [
                      "https://rir2.example.com/myrdap/"
                    ]
                  ],
                  [
                    ["2001:db8:4000::/36", "2001:db8:ffff::/48"],
                    [
                      "https://example.org/"
                    ]
                  ],
                  [
                    ["2001:db8:1000::/36"],
                    [
                      "https://example.net/rdaprir2/",
                      "http://example.net/rdaprir2/"
                    ]
                  ]
                ]
            }
        "#;
        let iana =
            serde_json::from_str::<IanaRegistry>(bootstrap).expect("cannot parse ipv6 bootstrap");

        // WHEN
        let actual =
            get_ipv6_bootstrap_urls(iana, &QueryType::IpV6Cidr("2001:db8:4000::/36".to_string()));

        // THEN
        assert_eq!(
            actual.expect("no vec").first().expect("vec is empty"),
            "https://example.org/"
        );
    }

    #[test]
    fn GIVEN_tag_bootstrap_with_match_WHEN_find_with_tag_THEN_return_match() {
        // GIVEN
        let bootstrap = r#"
            {
              "version": "1.0",
              "publication": "YYYY-MM-DDTHH:MM:SSZ",
              "description": "RDAP bootstrap file for service provider object tags",
              "services": [
                [
                  ["contact@example.com"],
                  ["YYYY"],
                  [
                    "https://example.com/rdap/"
                  ]
                ],
                [
                  ["contact@example.org"],
                  ["ZZ54"],
                  [
                    "http://rdap.example.org/"
                  ]
                ],
                [
                  ["contact@example.net"],
                  ["1754"],
                  [
                    "https://example.net/rdap/",
                    "http://example.net/rdap/"
                  ]
                ]
              ]
             }
        "#;
        let iana =
            serde_json::from_str::<IanaRegistry>(bootstrap).expect("cannot parse tag bootstrap");

        // WHEN
        let actual = get_tag_bootstrap_urls(iana, "YYYY");

        // THEN
        assert_eq!(
            actual.expect("no vec").first().expect("vec is empty"),
            "https://example.com/rdap/"
        );
    }
}
