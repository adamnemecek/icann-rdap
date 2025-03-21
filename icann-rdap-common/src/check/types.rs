use std::{any::TypeId, str::FromStr};

use crate::{
    media_types::RDAP_MEDIA_TYPE,
    response::{
        autnum::Autnum,
        domain::Domain,
        entity::Entity,
        nameserver::Nameserver,
        network::Network,
        types::{
            Common, ExtensionId, Link, Links, NoticeOrRemark, Notices, ObjectCommon, PublicIds,
            RdapConformance, Remarks, StringOrStringArray,
        },
    },
};
use chrono::DateTime;
use lazy_static::lazy_static;

use super::{
    string::{StringCheck, StringListCheck},
    Check, CheckItem, CheckParams, Checks, GetChecks, GetSubChecks,
};

impl GetChecks for RdapConformance {
    fn get_checks(&self, params: CheckParams) -> Checks {
        let mut items = vec![];
        if params.parent_type != params.root.get_type() {
            items.push(Check::RdapConformanceInvalidParent.check_item())
        };
        for ext in self {
            if !params.allow_unreg_ext {
                let id = ExtensionId::from_str(ext);
                if id.is_err() {
                    items.push(Check::UnknownExtention.check_item())
                }
            }
        }
        Checks {
            rdap_struct: super::RdapStructure::RdapConformance,
            items,
            sub_checks: vec![],
        }
    }
}

impl GetChecks for Links {
    fn get_checks(&self, params: CheckParams) -> Checks {
        let mut sub_checks: Vec<Checks> = vec![];
        if params.do_subchecks {
            self.iter()
                .for_each(|link| sub_checks.push(link.get_checks(params)));
        }
        Checks {
            rdap_struct: super::RdapStructure::Links,
            items: vec![],
            sub_checks,
        }
    }
}

lazy_static! {
    static ref RELATED_AND_SELF_LINK_PARENTS: Vec<TypeId> = vec![
        TypeId::of::<Domain>(),
        TypeId::of::<Entity>(),
        TypeId::of::<Autnum>(),
        TypeId::of::<Network>(),
    ];
}

impl GetChecks for Link {
    fn get_checks(&self, params: CheckParams) -> Checks {
        let mut items: Vec<CheckItem> = vec![];
        if self.value.is_none() {
            items.push(Check::LinkMissingValueProperty.check_item())
        };
        if self.href.is_none() {
            items.push(Check::LinkMissingHrefProperty.check_item())
        };
        if let Some(rel) = &self.rel {
            if rel.eq("related") {
                if let Some(media_type) = &self.media_type {
                    if !media_type.eq(RDAP_MEDIA_TYPE)
                        && RELATED_AND_SELF_LINK_PARENTS.contains(&params.parent_type)
                    {
                        items.push(Check::LinkRelatedIsNotRdap.check_item())
                    }
                } else {
                    items.push(Check::LinkRelatedHasNoType.check_item())
                }
            } else if rel.eq("self") {
                if let Some(media_type) = &self.media_type {
                    if !media_type.eq(RDAP_MEDIA_TYPE) {
                        items.push(Check::LinkSelfIsNotRdap.check_item())
                    }
                } else {
                    items.push(Check::LinkSelfHasNoType.check_item())
                }
            } else if RELATED_AND_SELF_LINK_PARENTS.contains(&params.parent_type) &&
                // because some registries do not model nameservers directly,
                // they can be embedded in other objects but aren't first class
                // objects themself (see RIR example in RFC 9083). Therefore,
                // it only matters that a nameserver has no self link if it is
                // the top most object (i.e. a first class object).
                params.root.get_type() != TypeId::of::<Nameserver>()
            {
                items.push(Check::LinkObjectClassHasNoSelf.check_item())
            }
        } else {
            items.push(Check::LinkMissingRelProperty.check_item())
        }
        Checks {
            rdap_struct: super::RdapStructure::Link,
            items,
            sub_checks: vec![],
        }
    }
}

impl GetChecks for Notices {
    fn get_checks(&self, params: CheckParams) -> Checks {
        let mut sub_checks: Vec<Checks> = vec![];
        if params.do_subchecks {
            self.iter()
                .for_each(|note| sub_checks.push(note.0.get_checks(params)));
        }
        Checks {
            rdap_struct: super::RdapStructure::Notices,
            items: vec![],
            sub_checks,
        }
    }
}

impl GetChecks for Remarks {
    fn get_checks(&self, params: CheckParams) -> Checks {
        let mut sub_checks: Vec<Checks> = vec![];
        if params.do_subchecks {
            self.iter()
                .for_each(|remark| sub_checks.push(remark.0.get_checks(params)));
        }
        Checks {
            rdap_struct: super::RdapStructure::Remarks,
            items: vec![],
            sub_checks,
        }
    }
}

impl GetChecks for NoticeOrRemark {
    fn get_checks(&self, params: CheckParams) -> Checks {
        let mut items: Vec<CheckItem> = vec![];
        if let Some(description) = &self.description {
            if matches!(description, StringOrStringArray::One(_)) {
                items.push(Check::NoticeOrRemarkDescriptionIsString.check_item())
            }
        } else {
            items.push(Check::NoticeOrRemarkDescriptionIsAbsent.check_item())
        };
        let mut sub_checks: Vec<Checks> = vec![];
        if params.do_subchecks {
            if let Some(links) = &self.links {
                links.iter().for_each(|link| {
                    sub_checks
                        .push(link.get_checks(params.from_parent(TypeId::of::<NoticeOrRemark>())))
                });
            };
        };
        Checks {
            rdap_struct: super::RdapStructure::NoticeOrRemark,
            items,
            sub_checks,
        }
    }
}

impl GetSubChecks for PublicIds {
    fn get_sub_checks(&self, _params: CheckParams) -> Vec<Checks> {
        let mut sub_checks: Vec<Checks> = vec![];
        self.iter().for_each(|pid| {
            if pid.id_type.is_none() {
                sub_checks.push(Checks {
                    rdap_struct: super::RdapStructure::PublidIds,
                    items: vec![Check::PublicIdTypeIsAbsent.check_item()],
                    sub_checks: vec![],
                });
            }
            if pid.identifier.is_none() {
                sub_checks.push(Checks {
                    rdap_struct: super::RdapStructure::PublidIds,
                    items: vec![Check::PublicIdIdentifierIsAbsent.check_item()],
                    sub_checks: vec![],
                });
            }
        });
        sub_checks
    }
}

impl GetSubChecks for Common {
    fn get_sub_checks(&self, params: CheckParams) -> Vec<Checks> {
        let mut sub_checks: Vec<Checks> = vec![];
        if params.do_subchecks {
            if let Some(rdap_conformance) = &self.rdap_conformance {
                sub_checks.push(rdap_conformance.get_checks(params))
            };
            if let Some(notices) = &self.notices {
                sub_checks.push(notices.get_checks(params))
            };
        };
        if params.parent_type == params.root.get_type() && self.rdap_conformance.is_none() {
            sub_checks.push(Checks {
                rdap_struct: super::RdapStructure::RdapConformance,
                items: vec![Check::RdapConformanceMissing.check_item()],
                sub_checks: vec![],
            });
        }
        sub_checks
    }
}

impl GetSubChecks for ObjectCommon {
    fn get_sub_checks(&self, params: CheckParams) -> Vec<Checks> {
        let mut sub_checks: Vec<Checks> = vec![];

        // entities
        if params.do_subchecks {
            if let Some(entities) = &self.entities {
                entities
                    .iter()
                    .for_each(|e| sub_checks.push(e.get_checks(params)))
            };
        }

        // links
        if let Some(links) = &self.links {
            sub_checks.push(links.get_checks(params));
        } else if params.root.get_type() != TypeId::of::<Nameserver>()
            && params.parent_type != TypeId::of::<Nameserver>()
        // because some registries do not model nameservers directly,
        // they can be embedded in other objects but aren't first class
        // objects themself (see RIR example in RFC 9083). Therefore,
        // it only matters that a nameserver has no self link if it is
        // the top most object (i.e. a first class object).
        {
            sub_checks.push(Checks {
                rdap_struct: super::RdapStructure::Links,
                items: vec![Check::LinkObjectClassHasNoSelf.check_item()],
                sub_checks: vec![],
            })
        };

        // remarks
        if let Some(remarks) = &self.remarks {
            sub_checks.push(remarks.get_checks(params))
        };

        // events
        if let Some(events) = &self.events {
            events.iter().for_each(|e| {
                if let Some(date) = &e.event_date {
                    let date = DateTime::parse_from_rfc3339(date);
                    if date.is_err() {
                        sub_checks.push(Checks {
                            rdap_struct: super::RdapStructure::Events,
                            items: vec![Check::EventDateIsNotRfc3339.check_item()],
                            sub_checks: vec![],
                        })
                    }
                } else {
                    sub_checks.push(Checks {
                        rdap_struct: super::RdapStructure::Events,
                        items: vec![Check::EventDateIsAbsent.check_item()],
                        sub_checks: vec![],
                    })
                }
                if e.event_action.is_none() {
                    sub_checks.push(Checks {
                        rdap_struct: super::RdapStructure::Events,
                        items: vec![Check::EventActionIsAbsent.check_item()],
                        sub_checks: vec![],
                    })
                }
            });
        }

        // handle
        if let Some(handle) = &self.handle {
            if handle.is_whitespace_or_empty() {
                sub_checks.push(Checks {
                    rdap_struct: super::RdapStructure::Handle,
                    items: vec![Check::HandleIsEmpty.check_item()],
                    sub_checks: vec![],
                })
            }
        }

        // Status
        if let Some(status) = &self.status {
            let status: Vec<&str> = status.iter().map(|s| s.0.as_str()).collect();
            if status.as_slice().is_empty_or_any_empty_or_whitespace() {
                sub_checks.push(Checks {
                    rdap_struct: super::RdapStructure::Status,
                    items: vec![Check::StatusIsEmpty.check_item()],
                    sub_checks: vec![],
                })
            }
        }

        // Port 43
        if let Some(port43) = &self.port_43 {
            if port43.is_whitespace_or_empty() {
                sub_checks.push(Checks {
                    rdap_struct: super::RdapStructure::Port43,
                    items: vec![Check::Port43IsEmpty.check_item()],
                    sub_checks: vec![],
                })
            }
        }

        sub_checks
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use rstest::rstest;

    use crate::{
        check::Checks,
        response::{
            domain::Domain,
            entity::Entity,
            nameserver::Nameserver,
            types::{
                Common, Event, Extension, Link, Notice, NoticeOrRemark, ObjectCommon, PublicId,
                Remark, StatusValue,
            },
            RdapResponse,
        },
    };

    use crate::check::{Check, CheckParams, GetChecks};

    #[test]
    fn GIVEN_link_with_no_rel_property_WHEN_checked_THEN_link_missing_rel_property() {
        // GIVEN
        let rdap = RdapResponse::Domain(
            Domain::builder()
                .common(Common::builder().build())
                .object_common(
                    ObjectCommon::domain()
                        .links(vec![Link {
                            href: Some("https://foo".to_string()),
                            value: Some("https://foo".to_string()),
                            rel: None,
                            title: None,
                            hreflang: None,
                            media: None,
                            media_type: None,
                        }])
                        .build(),
                )
                .build(),
        );

        // WHEN
        let checks = rdap.get_checks(CheckParams::for_rdap(&rdap));

        // THEN
        checks
            .sub(crate::check::RdapStructure::Links)
            .expect("Links not found")
            .sub(crate::check::RdapStructure::Link)
            .expect("Link not found")
            .items
            .iter()
            .find(|c| c.check == Check::LinkMissingRelProperty)
            .expect("link rel missing check");
    }

    #[test]
    fn GIVEN_link_with_no_val_property_WHEN_checked_THEN_link_missing_val_property() {
        // GIVEN
        let rdap = RdapResponse::Domain(
            Domain::builder()
                .common(Common::builder().build())
                .object_common(
                    ObjectCommon::domain()
                        .links(vec![Link {
                            href: Some("https://foo".to_string()),
                            value: None,
                            rel: Some("about".to_string()),
                            title: None,
                            hreflang: None,
                            media: None,
                            media_type: None,
                        }])
                        .build(),
                )
                .build(),
        );

        // WHEN
        let checks = rdap.get_checks(CheckParams::for_rdap(&rdap));

        // THEN
        checks
            .sub(crate::check::RdapStructure::Links)
            .expect("Links not found")
            .sub(crate::check::RdapStructure::Link)
            .expect("Link not found")
            .items
            .iter()
            .find(|c| c.check == Check::LinkMissingValueProperty)
            .expect("link missing check");
    }

    #[test]
    fn GIVEN_link_with_no_href_property_WHEN_checked_THEN_link_missing_href_property() {
        // GIVEN
        let rdap = RdapResponse::Domain(
            Domain::builder()
                .common(Common::builder().build())
                .object_common(
                    ObjectCommon::domain()
                        .links(vec![Link {
                            value: Some("https://foo".to_string()),
                            href: None,
                            rel: Some("about".to_string()),
                            title: None,
                            hreflang: None,
                            media: None,
                            media_type: None,
                        }])
                        .build(),
                )
                .build(),
        );

        // WHEN
        let checks = rdap.get_checks(CheckParams::for_rdap(&rdap));

        // THEN
        checks
            .sub(crate::check::RdapStructure::Links)
            .expect("Links not found")
            .sub(crate::check::RdapStructure::Link)
            .expect("Link not found")
            .items
            .iter()
            .find(|c| c.check == Check::LinkMissingHrefProperty)
            .expect("link missing check");
    }

    #[test]
    fn GIVEN_related_link_with_no_type_property_WHEN_checked_THEN_related_link_has_no_type() {
        // GIVEN
        let rdap = RdapResponse::Domain(
            Domain::builder()
                .common(Common::builder().build())
                .object_common(
                    ObjectCommon::domain()
                        .links(vec![Link::builder()
                            .href("https://foo")
                            .value("https://foo")
                            .rel("related")
                            .build()])
                        .build(),
                )
                .build(),
        );

        // WHEN
        let checks = rdap.get_checks(CheckParams::for_rdap(&rdap));

        // THEN
        checks
            .sub(crate::check::RdapStructure::Links)
            .expect("Links not found")
            .sub(crate::check::RdapStructure::Link)
            .expect("Link not found")
            .items
            .iter()
            .find(|c| c.check == Check::LinkRelatedHasNoType)
            .expect("link missing check");
    }

    #[test]
    fn GIVEN_object_related_link_with_non_rdap_type_WHEN_checked_THEN_related_link_not_rdap() {
        // GIVEN
        let rdap = RdapResponse::Domain(
            Domain::builder()
                .common(Common::builder().build())
                .object_common(
                    ObjectCommon::domain()
                        .links(vec![Link::builder()
                            .href("https://foo")
                            .value("https://foo")
                            .rel("related")
                            .media_type("foo")
                            .build()])
                        .build(),
                )
                .build(),
        );

        // WHEN
        let checks = rdap.get_checks(CheckParams::for_rdap(&rdap));

        // THEN
        checks
            .sub(crate::check::RdapStructure::Links)
            .expect("Links not found")
            .sub(crate::check::RdapStructure::Link)
            .expect("Link not found")
            .items
            .iter()
            .find(|c| c.check == Check::LinkRelatedIsNotRdap)
            .expect("link missing check");
    }

    #[test]
    fn GIVEN_self_link_with_no_type_property_WHEN_checked_THEN_self_link_has_no_type() {
        // GIVEN
        let rdap = RdapResponse::Domain(
            Domain::builder()
                .common(Common::builder().build())
                .object_common(
                    ObjectCommon::domain()
                        .links(vec![Link::builder()
                            .href("https://foo")
                            .value("https://foo")
                            .rel("self")
                            .build()])
                        .build(),
                )
                .build(),
        );

        // WHEN
        let checks = rdap.get_checks(CheckParams::for_rdap(&rdap));

        // THEN
        checks
            .sub(crate::check::RdapStructure::Links)
            .expect("Links not found")
            .sub(crate::check::RdapStructure::Link)
            .expect("Link not found")
            .items
            .iter()
            .find(|c| c.check == Check::LinkSelfHasNoType)
            .expect("link missing check");
    }

    #[test]
    fn GIVEN_self_link_with_non_rdap_type_WHEN_checked_THEN_self_link_not_rdap() {
        // GIVEN
        let rdap = RdapResponse::Domain(
            Domain::builder()
                .common(Common::builder().build())
                .object_common(
                    ObjectCommon::domain()
                        .links(vec![Link::builder()
                            .href("https://foo")
                            .value("https://foo")
                            .rel("self")
                            .media_type("foo")
                            .build()])
                        .build(),
                )
                .build(),
        );

        // WHEN
        let checks = rdap.get_checks(CheckParams::for_rdap(&rdap));

        // THEN
        assert!(find_any_check(&checks, Check::LinkSelfIsNotRdap));
    }

    #[test]
    fn GIVEN_domain_with_self_link_WHEN_checked_THEN_no_check_found() {
        // GIVEN
        let rdap = RdapResponse::Domain(
            Domain::builder()
                .common(Common::builder().build())
                .object_common(
                    ObjectCommon::domain()
                        .links(vec![Link::builder()
                            .href("https://foo")
                            .value("https://foo")
                            .rel("self")
                            .media_type("application/rdap+json")
                            .build()])
                        .build(),
                )
                .build(),
        );

        // WHEN
        let checks = rdap.get_checks(CheckParams::for_rdap(&rdap));

        // THEN
        dbg!(&checks);
        assert!(!find_any_check(&checks, Check::LinkObjectClassHasNoSelf));
    }

    #[test]
    fn GIVEN_nameserver_with_self_link_WHEN_checked_THEN_no_check_found() {
        // GIVEN
        let rdap = RdapResponse::Nameserver(
            Nameserver::builder()
                .common(Common::builder().build())
                .object_common(
                    ObjectCommon::domain()
                        .links(vec![Link::builder()
                            .href("https://foo")
                            .value("https://foo")
                            .rel("self")
                            .media_type("application/rdap+json")
                            .build()])
                        .build(),
                )
                .build(),
        );

        // WHEN
        let checks = rdap.get_checks(CheckParams::for_rdap(&rdap));

        // THEN
        assert!(!find_any_check(&checks, Check::LinkObjectClassHasNoSelf));
    }

    #[test]
    /// Issue #59
    fn GIVEN_nameserver_with_self_link_and_notice_WHEN_checked_THEN_no_check_found() {
        // GIVEN
        let rdap = RdapResponse::Nameserver(
            Nameserver::builder()
                .common(
                    Common::builder()
                        .notices(vec![Notice(
                            NoticeOrRemark::builder()
                                .description_entry("a notice")
                                .links(vec![Link::builder()
                                    .href("https://tos")
                                    .value("https://tos")
                                    .rel("terms-of-service")
                                    .media_type("text/html")
                                    .build()])
                                .build(),
                        )])
                        .build(),
                )
                .object_common(
                    ObjectCommon::domain()
                        .links(vec![Link::builder()
                            .href("https://foo")
                            .value("https://foo")
                            .rel("self")
                            .media_type("application/rdap+json")
                            .build()])
                        .build(),
                )
                .build(),
        );

        // WHEN
        let checks = rdap.get_checks(CheckParams::for_rdap(&rdap));

        // THEN
        dbg!(&checks);
        assert!(!find_any_check(&checks, Check::LinkObjectClassHasNoSelf));
    }

    #[test]
    /// Issue #59
    fn GIVEN_nameserver_with_self_link_and_remark_WHEN_checked_THEN_no_check_found() {
        // GIVEN
        let rdap = RdapResponse::Nameserver(
            Nameserver::builder()
                .common(Common::builder().build())
                .object_common(
                    ObjectCommon::domain()
                        .remarks(vec![Remark(
                            NoticeOrRemark::builder()
                                .description_entry("a notice")
                                .links(vec![Link::builder()
                                    .href("https://tos")
                                    .value("https://tos")
                                    .rel("terms-of-service")
                                    .media_type("text/html")
                                    .build()])
                                .build(),
                        )])
                        .links(vec![Link::builder()
                            .href("https://foo")
                            .value("https://foo")
                            .rel("self")
                            .media_type("application/rdap+json")
                            .build()])
                        .build(),
                )
                .build(),
        );

        // WHEN
        let checks = rdap.get_checks(CheckParams::for_rdap(&rdap));

        // THEN
        dbg!(&checks);
        assert!(!find_any_check(&checks, Check::LinkObjectClassHasNoSelf));
    }

    #[test]
    fn GIVEN_domain_with_no_self_link_WHEN_checked_THEN_object_classes_should_have_self_link() {
        // GIVEN
        let rdap = RdapResponse::Domain(
            Domain::builder()
                .common(Common::builder().build())
                .object_common(
                    ObjectCommon::domain()
                        .links(vec![Link::builder()
                            .href("https://foo")
                            .value("https://foo")
                            .rel("no_self")
                            .media_type("foo")
                            .build()])
                        .build(),
                )
                .build(),
        );

        // WHEN
        let checks = rdap.get_checks(CheckParams::for_rdap(&rdap));

        // THEN
        checks
            .sub(crate::check::RdapStructure::Links)
            .expect("Links not found")
            .sub(crate::check::RdapStructure::Link)
            .expect("Link not found")
            .items
            .iter()
            .find(|c| c.check == Check::LinkObjectClassHasNoSelf)
            .expect("link missing check");
    }

    #[test]
    fn GIVEN_domain_with_no_links_WHEN_checked_THEN_object_classes_should_have_self_link() {
        // GIVEN
        let rdap = RdapResponse::Domain(
            Domain::builder()
                .common(Common::builder().build())
                .object_common(ObjectCommon::domain().build())
                .build(),
        );

        // WHEN
        let checks = rdap.get_checks(CheckParams::for_rdap(&rdap));

        // THEN
        checks
            .sub(crate::check::RdapStructure::Links)
            .expect("Links not found")
            .items
            .iter()
            .find(|c| c.check == Check::LinkObjectClassHasNoSelf)
            .expect("link missing check");
    }

    #[test]
    fn GIVEN_event_with_no_date_WHEN_checked_THEN_event_date_absent() {
        // GIVEN
        let rdap = RdapResponse::Domain(
            Domain::builder()
                .common(Common::builder().build())
                .object_common(
                    ObjectCommon::domain()
                        .events(vec![Event {
                            event_action: Some("foo".to_string()),
                            event_date: None,
                            event_actor: None,
                            links: None,
                        }])
                        .build(),
                )
                .build(),
        );

        // WHEN
        let checks = rdap.get_checks(CheckParams::for_rdap(&rdap));

        // THEN
        checks
            .sub(crate::check::RdapStructure::Events)
            .expect("Events not found")
            .items
            .iter()
            .find(|c| c.check == Check::EventDateIsAbsent)
            .expect("event missing check");
    }

    #[test]
    fn GIVEN_event_with_no_action_WHEN_checked_THEN_event_action_absent() {
        // GIVEN
        let rdap = RdapResponse::Domain(
            Domain::builder()
                .common(Common::builder().build())
                .object_common(
                    ObjectCommon::domain()
                        .events(vec![Event {
                            event_date: Some("1990-12-31T23:59:59Z".to_string()),
                            event_action: None,
                            event_actor: None,
                            links: None,
                        }])
                        .build(),
                )
                .build(),
        );

        // WHEN
        let checks = rdap.get_checks(CheckParams::for_rdap(&rdap));

        // THEN
        checks
            .sub(crate::check::RdapStructure::Events)
            .expect("Events not found")
            .items
            .iter()
            .find(|c| c.check == Check::EventActionIsAbsent)
            .expect("event missing check");
    }

    #[test]
    fn GIVEN_event_with_bad_date_WHEN_checked_THEN_event_date_is_not_date() {
        // GIVEN
        let rdap = RdapResponse::Domain(
            Domain::builder()
                .common(Common::builder().build())
                .object_common(
                    ObjectCommon::domain()
                        .events(vec![Event::builder()
                            .event_action("foo")
                            .event_date("bar")
                            .build()])
                        .build(),
                )
                .build(),
        );

        // WHEN
        let checks = rdap.get_checks(CheckParams::for_rdap(&rdap));

        // THEN
        checks
            .sub(crate::check::RdapStructure::Events)
            .expect("Events not found")
            .items
            .iter()
            .find(|c| c.check == Check::EventDateIsNotRfc3339)
            .expect("event missing check");
    }

    #[test]
    fn GIVEN_public_id_with_no_type_WHEN_checked_THEN_type_is_absent() {
        // GIVEN
        let rdap = RdapResponse::Domain(
            Domain::builder()
                .common(Common::builder().build())
                .object_common(ObjectCommon::domain().build())
                .public_ids(vec![PublicId {
                    id_type: None,
                    identifier: Some("thing".to_string()),
                }])
                .build(),
        );

        // WHEN
        let checks = rdap.get_checks(CheckParams::for_rdap(&rdap));

        // THEN
        checks
            .sub(crate::check::RdapStructure::PublidIds)
            .expect("Public Ids not found")
            .items
            .iter()
            .find(|c| c.check == Check::PublicIdTypeIsAbsent)
            .expect("public id missing check");
    }

    #[test]
    fn GIVEN_public_id_with_no_identifier_WHEN_checked_THEN_identifier_is_absent() {
        // GIVEN
        let rdap = RdapResponse::Domain(
            Domain::builder()
                .common(Common::builder().build())
                .object_common(ObjectCommon::domain().build())
                .public_ids(vec![PublicId {
                    identifier: None,
                    id_type: Some("thing".to_string()),
                }])
                .build(),
        );

        // WHEN
        let checks = rdap.get_checks(CheckParams::for_rdap(&rdap));

        // THEN
        checks
            .sub(crate::check::RdapStructure::PublidIds)
            .expect("Public Ids not found")
            .items
            .iter()
            .find(|c| c.check == Check::PublicIdIdentifierIsAbsent)
            .expect("public id missing check");
    }

    #[test]
    fn GIVEN_notice_with_no_description_WHEN_checked_THEN_description_absent() {
        // GIVEN
        let notice = NoticeOrRemark {
            title: None,
            description: None,
            links: None,
        };
        let rdap = RdapResponse::Domain(
            Domain::builder()
                .common(Common::builder().notices(vec![Notice(notice)]).build())
                .object_common(ObjectCommon::domain().build())
                .build(),
        );

        // WHEN
        let checks = rdap.get_checks(CheckParams::for_rdap(&rdap));

        // THEN
        dbg!(&checks);
        checks
            .sub(crate::check::RdapStructure::Notices)
            .expect("Notices not found")
            .sub(crate::check::RdapStructure::NoticeOrRemark)
            .expect("Notice/Remark not found")
            .items
            .iter()
            .find(|c| c.check == Check::NoticeOrRemarkDescriptionIsAbsent)
            .expect("description missing check");
    }

    #[test]
    fn GIVEN_nameserver_with_no_links_WHEN_checked_THEN_no_object_classes_should_have_self_link() {
        // GIVEN
        let rdap = RdapResponse::Nameserver(
            Nameserver::builder()
                .common(Common::builder().build())
                .object_common(ObjectCommon::nameserver().build())
                .build(),
        );

        // WHEN
        let checks = rdap.get_checks(CheckParams::for_rdap(&rdap));

        // THEN
        assert!(checks.sub(crate::check::RdapStructure::Links).is_none());
    }

    #[test]
    fn GIVEN_nameserver_with_no_self_links_WHEN_checked_THEN_no_object_classes_should_have_self_link(
    ) {
        // GIVEN
        let rdap = RdapResponse::Nameserver(
            Nameserver::builder()
                .common(Common::builder().build())
                .object_common(
                    ObjectCommon::nameserver()
                        .links(vec![Link::builder()
                            .href("https://foo")
                            .value("https://foo")
                            .rel("no_self")
                            .media_type("foo")
                            .build()])
                        .build(),
                )
                .build(),
        );

        // WHEN
        let checks = rdap.get_checks(CheckParams::for_rdap(&rdap));

        // THEN
        assert!(!checks
            .sub(crate::check::RdapStructure::Links)
            .expect("Links not found")
            .items
            .iter()
            .any(|c| c.check == Check::LinkObjectClassHasNoSelf));
    }

    #[rstest]
    #[case(vec![])]
    #[case(vec![StatusValue("".to_string())])]
    #[case(vec![StatusValue("  ".to_string())])]
    #[case(vec![StatusValue("  ".to_string()), StatusValue("foo".to_string())])]
    #[test]
    fn GIVEN_nameserver_with_empty_status_WHEN_checked_THEN_status_is_empty(
        #[case] status: Vec<StatusValue>,
    ) {
        // GIVEN
        let mut ns = Nameserver::basic()
            .ldh_name("ns1.example.com")
            .build()
            .unwrap();
        ns.object_common.status = Some(status);
        let rdap = RdapResponse::Nameserver(ns);

        // WHEN
        let checks = rdap.get_checks(CheckParams::for_rdap(&rdap));

        // THEN
        assert!(checks
            .sub(crate::check::RdapStructure::Status)
            .expect("status not found")
            .items
            .iter()
            .any(|c| c.check == Check::StatusIsEmpty));
    }

    #[rstest]
    #[case("")]
    #[case("  ")]
    #[test]
    fn GIVEN_nameserver_with_empty_handle_WHEN_checked_THEN_handle_is_empty(#[case] handle: &str) {
        // GIVEN
        let mut ns = Nameserver::basic()
            .ldh_name("ns1.example.com")
            .build()
            .unwrap();
        ns.object_common.handle = Some(handle.to_string());
        let rdap = RdapResponse::Nameserver(ns);

        // WHEN
        let checks = rdap.get_checks(CheckParams::for_rdap(&rdap));

        // THEN
        assert!(checks
            .sub(crate::check::RdapStructure::Handle)
            .expect("handle not found")
            .items
            .iter()
            .any(|c| c.check == Check::HandleIsEmpty));
    }

    #[test]
    fn GIVEN_rdap_conformance_not_in_root_WHEN_checked_THEN_invalid_rdap_conformance_parent() {
        // GIVEN
        let rdap = RdapResponse::Domain(
            Domain::builder()
                .common(
                    Common::builder()
                        .rdap_conformance(vec![Extension("foo".to_string())])
                        .build(),
                )
                .object_common(
                    ObjectCommon::domain()
                        .entities(vec![Entity::builder()
                            .common(
                                Common::builder()
                                    .rdap_conformance(vec![Extension("foo".to_string())])
                                    .build(),
                            )
                            .object_common(ObjectCommon::entity().build())
                            .build()])
                        .build(),
                )
                .build(),
        );

        // WHEN
        let checks = rdap.get_checks(CheckParams::for_rdap(&rdap));

        // THEN
        checks
            .sub(crate::check::RdapStructure::Entity)
            .expect("entity not found")
            .sub(crate::check::RdapStructure::RdapConformance)
            .expect("rdap conformance not found")
            .items
            .iter()
            .find(|c| c.check == Check::RdapConformanceInvalidParent)
            .expect("check missing");
    }

    fn find_any_check(checks: &Checks, check_type: Check) -> bool {
        if checks.items.iter().any(|c| c.check == check_type) {
            return true;
        }
        if checks
            .sub_checks
            .iter()
            .any(|c| find_any_check(c, check_type))
        {
            return true;
        }
        false
    }
}
