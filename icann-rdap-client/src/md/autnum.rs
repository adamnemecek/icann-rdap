use icann_rdap_common::response::autnum::Autnum;

use super::{to_header, MdOptions, ToMd};

impl ToMd for Autnum {
    fn to_md(
        &self,
        heading_level: usize,
        check_types: &[crate::check::CheckType],
        options: &MdOptions,
    ) -> String {
        let mut md = String::new();
        md.push_str(&self.common.to_md(heading_level, check_types, options));
        let header_text = if self.start_autnum.is_some() && self.end_autnum.is_some() {
            format!(
                "Autonomous Systems {}-{}",
                &self.start_autnum.unwrap(),
                &self.end_autnum.unwrap()
            )
        } else if let Some(start_autnum) = &self.start_autnum {
            format!("Autonomous System {start_autnum}")
        } else if let Some(handle) = &self.object_common.handle {
            format!("Autonomous System {handle}")
        } else if let Some(name) = &self.name {
            format!("Autonomous System {name}")
        } else {
            "Autonomous System".to_string()
        };
        md.push_str(&to_header(&header_text, heading_level, options));
        md.push_str(
            &self
                .object_common
                .to_md(heading_level, check_types, options),
        );
        md.push('\n');
        md
    }
}
