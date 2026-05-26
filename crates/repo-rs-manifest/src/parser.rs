// Copyright 2026 Sunbeam Studios
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::Error;
use crate::model::*;
use crate::validate::is_valid_path;
use quick_xml::Reader;
use quick_xml::events::Event;
use std::collections::HashSet;

/// Parse a manifest from an XML string.
///
/// This performs syntactic parsing only. To validate remote references and
/// other cross-element constraints, call [`validate_manifest`] afterwards.
///
/// # Errors
///
/// Returns an error if the XML is malformed or contains invalid data.
pub fn parse_manifest(xml: &str) -> Result<Manifest, Error> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut manifest = Manifest::default();
    let mut buf = Vec::new();
    let mut stack: Vec<StackFrame> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = decode_name(&reader, e.name())?;
                match name.as_str() {
                    "project" => {
                        let proj = parse_project(&reader, &e)?;
                        stack.push(StackFrame::Project(proj));
                    }
                    "extend-project" => {
                        let ext = parse_extend_project(&reader, &e)?;
                        stack.push(StackFrame::ExtendProject(ext));
                    }
                    "remote" => {
                        let remote = parse_remote(&reader, &e)?;
                        if manifest.remotes.contains_key(&remote.name) {
                            return Err(Error::DuplicateRemote(remote.name));
                        }
                        manifest.remotes.insert(remote.name.clone(), remote);
                    }
                    "default" => {
                        manifest.default = Some(parse_default(&reader, &e)?);
                    }
                    "remove-project" => {
                        manifest
                            .remove_projects
                            .push(parse_remove_project(&reader, &e)?);
                    }
                    "submanifest" => {
                        let sm = parse_submanifest(&reader, &e)?;
                        if manifest.submanifests.contains_key(&sm.name) {
                            return Err(Error::DuplicateSubmanifest(sm.name));
                        }
                        manifest.submanifests.insert(sm.name.clone(), sm);
                    }
                    "repo-hooks" => {
                        manifest.repo_hooks = Some(parse_repo_hooks(&reader, &e)?);
                    }
                    "superproject" => {
                        manifest.superproject = Some(parse_superproject(&reader, &e)?);
                    }
                    "contactinfo" => {
                        manifest.contactinfo = Some(parse_contactinfo(&reader, &e)?);
                    }
                    "manifest-server" => {
                        manifest.manifest_server = Some(parse_manifest_server(&reader, &e)?);
                    }
                    "include" => {
                        manifest.includes.push(parse_include(&reader, &e)?);
                    }
                    "annotation" => {
                        let ann = parse_annotation(&reader, &e)?;
                        if let Some(frame) = stack.last_mut() {
                            frame.push_annotation(ann);
                        }
                    }
                    "copyfile" => {
                        let cf = parse_copyfile(&reader, &e)?;
                        if let Some(frame) = stack.last_mut() {
                            frame.push_copyfile(cf);
                        }
                    }
                    "linkfile" => {
                        let lf = parse_linkfile(&reader, &e)?;
                        if let Some(frame) = stack.last_mut() {
                            frame.push_linkfile(lf);
                        }
                    }
                    "notice" => {
                        let text = read_text_content(&mut reader, &mut buf, "notice")?;
                        manifest.notice = Some(text);
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                let name = decode_name(&reader, e.name())?;
                match name.as_str() {
                    "remote" => {
                        let remote = parse_remote(&reader, &e)?;
                        if manifest.remotes.contains_key(&remote.name) {
                            return Err(Error::DuplicateRemote(remote.name));
                        }
                        manifest.remotes.insert(remote.name.clone(), remote);
                    }
                    "default" => {
                        manifest.default = Some(parse_default(&reader, &e)?);
                    }
                    "project" => {
                        let proj = parse_project(&reader, &e)?;
                        if let Some(StackFrame::Project(parent)) = stack.last_mut() {
                            parent.subprojects.push(proj);
                        } else {
                            manifest.projects.push(proj);
                        }
                    }
                    "extend-project" => {
                        let ext = parse_extend_project(&reader, &e)?;
                        // extend-project cannot nest; always add to top-level
                        manifest.extend_projects.push(ext);
                    }
                    "remove-project" => {
                        manifest
                            .remove_projects
                            .push(parse_remove_project(&reader, &e)?);
                    }
                    "submanifest" => {
                        let sm = parse_submanifest(&reader, &e)?;
                        if manifest.submanifests.contains_key(&sm.name) {
                            return Err(Error::DuplicateSubmanifest(sm.name));
                        }
                        manifest.submanifests.insert(sm.name.clone(), sm);
                    }
                    "repo-hooks" => {
                        manifest.repo_hooks = Some(parse_repo_hooks(&reader, &e)?);
                    }
                    "superproject" => {
                        manifest.superproject = Some(parse_superproject(&reader, &e)?);
                    }
                    "contactinfo" => {
                        manifest.contactinfo = Some(parse_contactinfo(&reader, &e)?);
                    }
                    "manifest-server" => {
                        manifest.manifest_server = Some(parse_manifest_server(&reader, &e)?);
                    }
                    "include" => {
                        manifest.includes.push(parse_include(&reader, &e)?);
                    }
                    "annotation" => {
                        let ann = parse_annotation(&reader, &e)?;
                        if let Some(frame) = stack.last_mut() {
                            frame.push_annotation(ann);
                        }
                    }
                    "copyfile" => {
                        let cf = parse_copyfile(&reader, &e)?;
                        if let Some(frame) = stack.last_mut() {
                            frame.push_copyfile(cf);
                        }
                    }
                    "linkfile" => {
                        let lf = parse_linkfile(&reader, &e)?;
                        if let Some(frame) = stack.last_mut() {
                            frame.push_linkfile(lf);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let name = decode_name(&reader, e.name())?;
                match name.as_str() {
                    "project" => {
                        if let Some(StackFrame::Project(proj)) = stack.pop() {
                            if let Some(StackFrame::Project(parent)) = stack.last_mut() {
                                parent.subprojects.push(proj);
                            } else {
                                manifest.projects.push(proj);
                            }
                        }
                    }
                    "extend-project" => {
                        if let Some(StackFrame::ExtendProject(ext)) = stack.pop() {
                            manifest.extend_projects.push(ext);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(Error::Xml(e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(manifest)
}

enum StackFrame {
    Project(Project),
    ExtendProject(ExtendProject),
}

impl StackFrame {
    fn push_annotation(&mut self, ann: Annotation) {
        match self {
            StackFrame::Project(p) => p.annotations.push(ann),
            StackFrame::ExtendProject(e) => e.annotations.push(ann),
        }
    }

    fn push_copyfile(&mut self, cf: CopyFile) {
        match self {
            StackFrame::Project(p) => p.copyfiles.push(cf),
            StackFrame::ExtendProject(e) => e.copyfiles.push(cf),
        }
    }

    fn push_linkfile(&mut self, lf: LinkFile) {
        match self {
            StackFrame::Project(p) => p.linkfiles.push(lf),
            StackFrame::ExtendProject(e) => e.linkfiles.push(lf),
        }
    }
}

fn decode_name(reader: &Reader<&[u8]>, name: quick_xml::name::QName<'_>) -> Result<String, Error> {
    Ok(reader.decoder().decode(name.as_ref())?.into_owned())
}

fn attr(
    reader: &Reader<&[u8]>,
    e: &quick_xml::events::BytesStart<'_>,
    name: &str,
) -> Result<Option<String>, Error> {
    for attribute in e.attributes() {
        let attribute = attribute.map_err(|e| Error::Xml(e.into()))?;
        let key = decode_name(reader, attribute.key)?;
        if key == name {
            let value = attribute
                .decoded_and_normalized_value(
                    quick_xml::XmlVersion::Implicit1_0,
                    reader.decoder(),
                )
                .map_err(Error::Xml)?;
            return Ok(Some(value.into_owned()));
        }
    }
    Ok(None)
}

fn required_attr(
    reader: &Reader<&[u8]>,
    e: &quick_xml::events::BytesStart<'_>,
    name: &str,
) -> Result<String, Error> {
    attr(reader, e, name)?.ok_or_else(|| Error::MissingAttribute(name.to_string()))
}

fn parse_groups(s: Option<&str>) -> HashSet<String> {
    match s {
        Some(s) => s
            .split(',')
            .map(|g| g.trim().to_string())
            .filter(|g| !g.is_empty())
            .collect(),
        None => HashSet::new(),
    }
}

fn parse_bool(s: Option<&str>) -> Option<bool> {
    s.map(|v| v == "true")
}

fn parse_u32(s: Option<&str>) -> Result<Option<u32>, Error> {
    match s {
        Some(v) => v
            .parse::<u32>()
            .map(Some)
            .map_err(|e: std::num::ParseIntError| Error::InvalidPath(e.to_string())),
        None => Ok(None),
    }
}

fn parse_remote(
    reader: &Reader<&[u8]>,
    e: &quick_xml::events::BytesStart<'_>,
) -> Result<Remote, Error> {
    Ok(Remote {
        name: required_attr(reader, e, "name")?,
        alias: attr(reader, e, "alias")?,
        fetch: required_attr(reader, e, "fetch")?,
        pushurl: attr(reader, e, "pushurl")?,
        review: attr(reader, e, "review")?,
        revision: attr(reader, e, "revision")?,
        annotations: Vec::new(),
    })
}

fn parse_default(
    reader: &Reader<&[u8]>,
    e: &quick_xml::events::BytesStart<'_>,
) -> Result<Default, Error> {
    Ok(Default {
        remote: attr(reader, e, "remote")?,
        revision: attr(reader, e, "revision")?,
        dest_branch: attr(reader, e, "dest-branch")?,
        upstream: attr(reader, e, "upstream")?,
        sync_j: parse_u32(attr(reader, e, "sync-j")?.as_deref())?,
        sync_j_max: parse_u32(attr(reader, e, "sync-j-max")?.as_deref())?,
        sync_c: parse_bool(attr(reader, e, "sync-c")?.as_deref()),
        sync_s: parse_bool(attr(reader, e, "sync-s")?.as_deref()),
        sync_tags: parse_bool(attr(reader, e, "sync-tags")?.as_deref()),
    })
}

fn parse_project(
    reader: &Reader<&[u8]>,
    e: &quick_xml::events::BytesStart<'_>,
) -> Result<Project, Error> {
    let name = required_attr(reader, e, "name")?;
    let path = attr(reader, e, "path")?;
    if let Some(ref p) = path {
        if !is_valid_path(p) {
            return Err(Error::InvalidPath(format!("invalid project path: {p}")));
        }
    }
    Ok(Project {
        name,
        path,
        remote: attr(reader, e, "remote")?,
        revision: attr(reader, e, "revision")?,
        dest_branch: attr(reader, e, "dest-branch")?,
        groups: parse_groups(attr(reader, e, "groups")?.as_deref()),
        sync_c: parse_bool(attr(reader, e, "sync-c")?.as_deref()),
        sync_s: parse_bool(attr(reader, e, "sync-s")?.as_deref()),
        sync_tags: parse_bool(attr(reader, e, "sync-tags")?.as_deref()),
        upstream: attr(reader, e, "upstream")?,
        clone_depth: parse_u32(attr(reader, e, "clone-depth")?.as_deref())?,
        force_path: parse_bool(attr(reader, e, "force-path")?.as_deref()),
        sync_strategy: attr(reader, e, "sync-strategy")?,
        annotations: Vec::new(),
        copyfiles: Vec::new(),
        linkfiles: Vec::new(),
        subprojects: Vec::new(),
    })
}

fn parse_extend_project(
    reader: &Reader<&[u8]>,
    e: &quick_xml::events::BytesStart<'_>,
) -> Result<ExtendProject, Error> {
    Ok(ExtendProject {
        name: required_attr(reader, e, "name")?,
        path: attr(reader, e, "path")?,
        dest_path: attr(reader, e, "dest-path")?,
        groups: parse_groups(attr(reader, e, "groups")?.as_deref()),
        revision: attr(reader, e, "revision")?,
        remote: attr(reader, e, "remote")?,
        dest_branch: attr(reader, e, "dest-branch")?,
        upstream: attr(reader, e, "upstream")?,
        base_rev: attr(reader, e, "base-rev")?,
        annotations: Vec::new(),
        copyfiles: Vec::new(),
        linkfiles: Vec::new(),
    })
}

fn parse_remove_project(
    reader: &Reader<&[u8]>,
    e: &quick_xml::events::BytesStart<'_>,
) -> Result<RemoveProject, Error> {
    Ok(RemoveProject {
        name: attr(reader, e, "name")?,
        path: attr(reader, e, "path")?,
        optional: parse_bool(attr(reader, e, "optional")?.as_deref()).unwrap_or(false),
        base_rev: attr(reader, e, "base-rev")?,
    })
}

fn parse_submanifest(
    reader: &Reader<&[u8]>,
    e: &quick_xml::events::BytesStart<'_>,
) -> Result<Submanifest, Error> {
    Ok(Submanifest {
        name: required_attr(reader, e, "name")?,
        remote: attr(reader, e, "remote")?,
        project: attr(reader, e, "project")?,
        manifest_name: attr(reader, e, "manifest-name")?,
        revision: attr(reader, e, "revision")?,
        path: attr(reader, e, "path")?,
        groups: parse_groups(attr(reader, e, "groups")?.as_deref()),
        default_groups: parse_groups(attr(reader, e, "default-groups")?.as_deref()),
    })
}

fn parse_repo_hooks(
    reader: &Reader<&[u8]>,
    e: &quick_xml::events::BytesStart<'_>,
) -> Result<RepoHooks, Error> {
    Ok(RepoHooks {
        in_project: required_attr(reader, e, "in-project")?,
        enabled_list: required_attr(reader, e, "enabled-list")?,
    })
}

fn parse_superproject(
    reader: &Reader<&[u8]>,
    e: &quick_xml::events::BytesStart<'_>,
) -> Result<Superproject, Error> {
    Ok(Superproject {
        name: required_attr(reader, e, "name")?,
        remote: attr(reader, e, "remote")?,
        revision: attr(reader, e, "revision")?,
    })
}

fn parse_contactinfo(
    reader: &Reader<&[u8]>,
    e: &quick_xml::events::BytesStart<'_>,
) -> Result<ContactInfo, Error> {
    Ok(ContactInfo {
        bugurl: required_attr(reader, e, "bugurl")?,
    })
}

fn parse_manifest_server(
    reader: &Reader<&[u8]>,
    e: &quick_xml::events::BytesStart<'_>,
) -> Result<String, Error> {
    required_attr(reader, e, "url")
}

fn parse_include(
    reader: &Reader<&[u8]>,
    e: &quick_xml::events::BytesStart<'_>,
) -> Result<Include, Error> {
    Ok(Include {
        name: required_attr(reader, e, "name")?,
        groups: parse_groups(attr(reader, e, "groups")?.as_deref()),
        revision: attr(reader, e, "revision")?,
    })
}

fn parse_annotation(
    reader: &Reader<&[u8]>,
    e: &quick_xml::events::BytesStart<'_>,
) -> Result<Annotation, Error> {
    Ok(Annotation {
        name: required_attr(reader, e, "name")?,
        value: required_attr(reader, e, "value")?,
        keep: attr(reader, e, "keep")?.as_deref() != Some("false"),
    })
}

fn parse_copyfile(
    reader: &Reader<&[u8]>,
    e: &quick_xml::events::BytesStart<'_>,
) -> Result<CopyFile, Error> {
    Ok(CopyFile {
        src: required_attr(reader, e, "src")?,
        dest: required_attr(reader, e, "dest")?,
    })
}

fn parse_linkfile(
    reader: &Reader<&[u8]>,
    e: &quick_xml::events::BytesStart<'_>,
) -> Result<LinkFile, Error> {
    Ok(LinkFile {
        src: required_attr(reader, e, "src")?,
        dest: required_attr(reader, e, "dest")?,
    })
}

fn resolve_general_ref(content: &str) -> Result<String, Error> {
    if let Some(ch) = quick_xml::escape::resolve_xml_entity(content) {
        return Ok(ch.to_string());
    }
    if let Some(rest) = content.strip_prefix('#') {
        let codepoint =
            if let Some(hex) = rest.strip_prefix('x').or_else(|| rest.strip_prefix('X')) {
                u32::from_str_radix(hex, 16)
            } else {
                rest.parse::<u32>()
            }
            .map_err(|e| Error::Encoding(format!("invalid character reference: {e}")))?;
        let ch = char::from_u32(codepoint)
            .ok_or_else(|| Error::Encoding("invalid character reference".to_string()))?;
        return Ok(ch.to_string());
    }
    Err(Error::Encoding(format!(
        "unknown entity reference: {content}"
    )))
}

fn read_text_content(
    reader: &mut Reader<&[u8]>,
    buf: &mut Vec<u8>,
    end_tag: &str,
) -> Result<String, Error> {
    let mut text = String::new();

    // Disable text trimming while reading mixed content so that spaces
    // around entity references are preserved.
    let old_trim_start = reader.config_mut().trim_text_start;
    let old_trim_end = reader.config_mut().trim_text_end;
    reader.config_mut().trim_text_start = false;
    reader.config_mut().trim_text_end = false;

    loop {
        match reader.read_event_into(buf) {
            Ok(Event::Text(e)) => {
                let decoded = reader.decoder().decode(e.as_ref())?;
                text.push_str(&decoded);
            }
            Ok(Event::GeneralRef(e)) => {
                let decoded = e.decode().map_err(|e| Error::Encoding(e.to_string()))?;
                text.push_str(&resolve_general_ref(&decoded)?);
            }
            Ok(Event::End(e)) => {
                let name = decode_name(reader, e.name())?;
                if name == end_tag {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(Error::Xml(e)),
            _ => {}
        }
        buf.clear();
    }

    reader.config_mut().trim_text_start = old_trim_start;
    reader.config_mut().trim_text_end = old_trim_end;

    Ok(text)
}

/// Validate a parsed manifest.
///
/// Checks that all project remote references refer to defined remotes.
///
/// # Errors
///
/// Returns an error if a project references an undefined remote.
pub fn validate_manifest(manifest: &Manifest) -> Result<(), Error> {
    for project in &manifest.projects {
        if let Some(ref remote_name) = project.remote {
            if !manifest.remotes.contains_key(remote_name) {
                return Err(Error::UnknownRemote(remote_name.clone()));
            }
        }
        for subproject in &project.subprojects {
            if let Some(ref remote_name) = subproject.remote {
                if !manifest.remotes.contains_key(remote_name) {
                    return Err(Error::UnknownRemote(remote_name.clone()));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_manifest() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://example.com" />
  <default remote="origin" revision="main" />
  <project name="foo" path="foo" />
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert_eq!(manifest.remotes.len(), 1);
        assert_eq!(manifest.remotes["origin"].fetch, "https://example.com");
        assert_eq!(manifest.projects.len(), 1);
        assert_eq!(manifest.projects[0].name, "foo");
        assert_eq!(manifest.projects[0].path, Some("foo".to_string()));
    }

    #[test]
    fn parse_manifest_with_groups() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://example.com" />
  <default remote="origin" revision="main" />
  <project name="foo" path="foo" groups="group1,group2" />
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert!(manifest.projects[0].groups.contains("group1"));
        assert!(manifest.projects[0].groups.contains("group2"));
    }

    #[test]
    fn parse_manifest_with_copyfile() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://example.com" />
  <default remote="origin" revision="main" />
  <project name="foo" path="foo">
    <copyfile src="src.txt" dest="dest.txt" />
  </project>
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert_eq!(manifest.projects[0].copyfiles.len(), 1);
        assert_eq!(manifest.projects[0].copyfiles[0].src, "src.txt");
    }

    #[test]
    fn parse_manifest_with_linkfile() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://example.com" />
  <default remote="origin" revision="main" />
  <project name="foo" path="foo">
    <linkfile src="src.txt" dest="dest.txt" />
  </project>
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert_eq!(manifest.projects[0].linkfiles.len(), 1);
        assert_eq!(manifest.projects[0].linkfiles[0].src, "src.txt");
    }

    #[test]
    fn parse_manifest_with_notice() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <notice>Hello World</notice>
  <remote name="origin" fetch="https://example.com" />
  <default remote="origin" revision="main" />
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert_eq!(manifest.notice, Some("Hello World".to_string()));
    }

    #[test]
    fn reject_invalid_remote() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <project name="foo" remote="unknown" />
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        let err = validate_manifest(&manifest).unwrap_err();
        assert!(
            err.to_string()
                .contains("unknown remote reference: unknown")
        );
    }

    #[test]
    fn parse_all_element_types() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <notice>Notice text</notice>
  <remote name="origin" alias="o" fetch="https://example.com" pushurl="https://push.example.com" review="https://review.example.com" revision="main" />
  <default remote="origin" revision="main" dest-branch="main" upstream="main" sync-j="4" sync-j-max="8" sync-c="true" sync-s="false" sync-tags="true" />
  <manifest-server url="https://manifest.example.com" />
  <contactinfo bugurl="https://bugs.example.com" />
  <superproject name="super" remote="origin" revision="main" />
  <repo-hooks in-project="hooks" enabled-list="enabled.txt" />
  <submanifest name="sub" remote="origin" project="subproj" manifest-name="sub.xml" revision="main" path="sub" groups="g1,g2" default-groups="dg1" />
  <include name="other.xml" groups="ig1" revision="main" />
  <project name="foo" path="foo" remote="origin" revision="main" dest-branch="main" upstream="main" groups="g1,g2" sync-c="true" sync-s="false" sync-tags="true" clone-depth="1" force-path="true" sync-strategy="auto">
    <annotation name="ann" value="val" keep="true" />
    <copyfile src="a" dest="b" />
    <linkfile src="c" dest="d" />
    <project name="subfoo" path="subfoo" />
  </project>
  <extend-project name="foo" path="bar" dest-path="baz" groups="eg1" revision="dev" remote="origin" dest-branch="dev" upstream="dev" base-rev="abc">
    <annotation name="eann" value="eval" keep="false" />
    <copyfile src="e" dest="f" />
    <linkfile src="g" dest="h" />
  </extend-project>
  <remove-project name="old" path="oldpath" optional="true" base-rev="def" />
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();

        // notice
        assert_eq!(manifest.notice, Some("Notice text".to_string()));

        // remote
        assert_eq!(manifest.remotes.len(), 1);
        let remote = &manifest.remotes["origin"];
        assert_eq!(remote.name, "origin");
        assert_eq!(remote.alias, Some("o".to_string()));
        assert_eq!(remote.fetch, "https://example.com");
        assert_eq!(remote.pushurl, Some("https://push.example.com".to_string()));
        assert_eq!(
            remote.review,
            Some("https://review.example.com".to_string())
        );
        assert_eq!(remote.revision, Some("main".to_string()));

        // default
        let default = manifest.default.as_ref().unwrap();
        assert_eq!(default.remote, Some("origin".to_string()));
        assert_eq!(default.revision, Some("main".to_string()));
        assert_eq!(default.dest_branch, Some("main".to_string()));
        assert_eq!(default.upstream, Some("main".to_string()));
        assert_eq!(default.sync_j, Some(4));
        assert_eq!(default.sync_j_max, Some(8));
        assert_eq!(default.sync_c, Some(true));
        assert_eq!(default.sync_s, Some(false));
        assert_eq!(default.sync_tags, Some(true));

        // manifest-server
        assert_eq!(
            manifest.manifest_server,
            Some("https://manifest.example.com".to_string())
        );

        // contactinfo
        let contact = manifest.contactinfo.as_ref().unwrap();
        assert_eq!(contact.bugurl, "https://bugs.example.com");

        // superproject
        let superproject = manifest.superproject.as_ref().unwrap();
        assert_eq!(superproject.name, "super");
        assert_eq!(superproject.remote, Some("origin".to_string()));
        assert_eq!(superproject.revision, Some("main".to_string()));

        // repo-hooks
        let hooks = manifest.repo_hooks.as_ref().unwrap();
        assert_eq!(hooks.in_project, "hooks");
        assert_eq!(hooks.enabled_list, "enabled.txt");

        // submanifest
        assert_eq!(manifest.submanifests.len(), 1);
        let sub = &manifest.submanifests["sub"];
        assert_eq!(sub.name, "sub");
        assert_eq!(sub.remote, Some("origin".to_string()));
        assert_eq!(sub.project, Some("subproj".to_string()));
        assert_eq!(sub.manifest_name, Some("sub.xml".to_string()));
        assert_eq!(sub.revision, Some("main".to_string()));
        assert_eq!(sub.path, Some("sub".to_string()));
        assert!(sub.groups.contains("g1"));
        assert!(sub.groups.contains("g2"));
        assert!(sub.default_groups.contains("dg1"));

        // include
        assert_eq!(manifest.includes.len(), 1);
        let inc = &manifest.includes[0];
        assert_eq!(inc.name, "other.xml");
        assert!(inc.groups.contains("ig1"));
        assert_eq!(inc.revision, Some("main".to_string()));

        // project
        assert_eq!(manifest.projects.len(), 1);
        let proj = &manifest.projects[0];
        assert_eq!(proj.name, "foo");
        assert_eq!(proj.path, Some("foo".to_string()));
        assert_eq!(proj.remote, Some("origin".to_string()));
        assert_eq!(proj.revision, Some("main".to_string()));
        assert_eq!(proj.dest_branch, Some("main".to_string()));
        assert_eq!(proj.upstream, Some("main".to_string()));
        assert!(proj.groups.contains("g1"));
        assert!(proj.groups.contains("g2"));
        assert_eq!(proj.sync_c, Some(true));
        assert_eq!(proj.sync_s, Some(false));
        assert_eq!(proj.sync_tags, Some(true));
        assert_eq!(proj.clone_depth, Some(1));
        assert_eq!(proj.force_path, Some(true));
        assert_eq!(proj.sync_strategy, Some("auto".to_string()));
        assert_eq!(proj.annotations.len(), 1);
        assert_eq!(proj.annotations[0].name, "ann");
        assert_eq!(proj.annotations[0].value, "val");
        assert_eq!(proj.annotations[0].keep, true);
        assert_eq!(proj.copyfiles.len(), 1);
        assert_eq!(proj.copyfiles[0].src, "a");
        assert_eq!(proj.copyfiles[0].dest, "b");
        assert_eq!(proj.linkfiles.len(), 1);
        assert_eq!(proj.linkfiles[0].src, "c");
        assert_eq!(proj.linkfiles[0].dest, "d");
        assert_eq!(proj.subprojects.len(), 1);
        assert_eq!(proj.subprojects[0].name, "subfoo");

        // extend-project
        assert_eq!(manifest.extend_projects.len(), 1);
        let ext = &manifest.extend_projects[0];
        assert_eq!(ext.name, "foo");
        assert_eq!(ext.path, Some("bar".to_string()));
        assert_eq!(ext.dest_path, Some("baz".to_string()));
        assert!(ext.groups.contains("eg1"));
        assert_eq!(ext.revision, Some("dev".to_string()));
        assert_eq!(ext.remote, Some("origin".to_string()));
        assert_eq!(ext.dest_branch, Some("dev".to_string()));
        assert_eq!(ext.upstream, Some("dev".to_string()));
        assert_eq!(ext.base_rev, Some("abc".to_string()));
        assert_eq!(ext.annotations.len(), 1);
        assert_eq!(ext.annotations[0].name, "eann");
        assert_eq!(ext.annotations[0].value, "eval");
        assert_eq!(ext.annotations[0].keep, false);
        assert_eq!(ext.copyfiles.len(), 1);
        assert_eq!(ext.copyfiles[0].src, "e");
        assert_eq!(ext.linkfiles.len(), 1);
        assert_eq!(ext.linkfiles[0].src, "g");

        // remove-project
        assert_eq!(manifest.remove_projects.len(), 1);
        let rem = &manifest.remove_projects[0];
        assert_eq!(rem.name, Some("old".to_string()));
        assert_eq!(rem.path, Some("oldpath".to_string()));
        assert_eq!(rem.optional, true);
        assert_eq!(rem.base_rev, Some("def".to_string()));
    }

    #[test]
    fn parse_empty_manifest() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert_eq!(manifest.projects.len(), 0);
        assert_eq!(manifest.remotes.len(), 0);
    }

    #[test]
    fn parse_manifest_with_whitespace_only_text() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://example.com" />
  <default remote="origin" revision="main" />
  <project name="foo" path="foo" />
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert_eq!(manifest.projects.len(), 1);
    }

    #[test]
    fn parse_notice_with_special_characters() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <notice>Special &lt;tags&gt; &amp; &quot;quotes&quot;</notice>
  <remote name="origin" fetch="https://example.com" />
  <default remote="origin" revision="main" />
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert_eq!(
            manifest.notice,
            Some("Special <tags> & \"quotes\"".to_string())
        );
    }

    #[test]
    fn parse_notice_with_multiline_text() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <notice>Line 1
Line 2
Line 3</notice>
  <remote name="origin" fetch="https://example.com" />
  <default remote="origin" revision="main" />
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert_eq!(manifest.notice, Some("Line 1\nLine 2\nLine 3".to_string()));
    }

    #[test]
    fn reject_missing_required_attribute_remote_name() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote fetch="https://example.com" />
</manifest>
"#;
        let err = parse_manifest(xml).unwrap_err();
        assert!(err.to_string().contains("missing required attribute: name"));
    }

    #[test]
    fn reject_missing_required_attribute_remote_fetch() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" />
</manifest>
"#;
        let err = parse_manifest(xml).unwrap_err();
        assert!(
            err.to_string()
                .contains("missing required attribute: fetch")
        );
    }

    #[test]
    fn reject_missing_required_attribute_project_name() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <project path="foo" />
</manifest>
"#;
        let err = parse_manifest(xml).unwrap_err();
        assert!(err.to_string().contains("missing required attribute: name"));
    }

    #[test]
    fn reject_missing_required_attribute_repo_hooks() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <repo-hooks in-project="hooks" />
</manifest>
"#;
        let err = parse_manifest(xml).unwrap_err();
        assert!(
            err.to_string()
                .contains("missing required attribute: enabled-list")
        );
    }

    #[test]
    fn reject_missing_required_attribute_superproject() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <superproject />
</manifest>
"#;
        let err = parse_manifest(xml).unwrap_err();
        assert!(err.to_string().contains("missing required attribute: name"));
    }

    #[test]
    fn reject_missing_required_attribute_contactinfo() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <contactinfo />
</manifest>
"#;
        let err = parse_manifest(xml).unwrap_err();
        assert!(
            err.to_string()
                .contains("missing required attribute: bugurl")
        );
    }

    #[test]
    fn reject_missing_required_attribute_manifest_server() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <manifest-server />
</manifest>
"#;
        let err = parse_manifest(xml).unwrap_err();
        assert!(err.to_string().contains("missing required attribute: url"));
    }

    #[test]
    fn reject_missing_required_attribute_include() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <include />
</manifest>
"#;
        let err = parse_manifest(xml).unwrap_err();
        assert!(err.to_string().contains("missing required attribute: name"));
    }

    #[test]
    fn reject_missing_required_attribute_annotation() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://example.com" />
  <default remote="origin" revision="main" />
  <project name="foo" path="foo">
    <annotation value="val" />
  </project>
</manifest>
"#;
        let err = parse_manifest(xml).unwrap_err();
        assert!(err.to_string().contains("missing required attribute: name"));
    }

    #[test]
    fn reject_missing_required_attribute_copyfile() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://example.com" />
  <default remote="origin" revision="main" />
  <project name="foo" path="foo">
    <copyfile dest="b" />
  </project>
</manifest>
"#;
        let err = parse_manifest(xml).unwrap_err();
        assert!(err.to_string().contains("missing required attribute: src"));
    }

    #[test]
    fn reject_missing_required_attribute_linkfile() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://example.com" />
  <default remote="origin" revision="main" />
  <project name="foo" path="foo">
    <linkfile src="a" />
  </project>
</manifest>
"#;
        let err = parse_manifest(xml).unwrap_err();
        assert!(err.to_string().contains("missing required attribute: dest"));
    }

    #[test]
    fn reject_duplicate_remote() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://example.com" />
  <remote name="origin" fetch="https://other.com" />
</manifest>
"#;
        let err = parse_manifest(xml).unwrap_err();
        assert!(err.to_string().contains("duplicate remote name: origin"));
    }

    #[test]
    fn reject_duplicate_submanifest() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <submanifest name="sub" />
  <submanifest name="sub" />
</manifest>
"#;
        let err = parse_manifest(xml).unwrap_err();
        assert!(err.to_string().contains("duplicate submanifest name: sub"));
    }

    #[test]
    fn reject_invalid_project_path() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <project name="foo" path="../foo" />
</manifest>
"#;
        let err = parse_manifest(xml).unwrap_err();
        assert!(err.to_string().contains("invalid project path"));
    }

    #[test]
    fn reject_unknown_remote_in_subproject() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://example.com" />
  <default remote="origin" revision="main" />
  <project name="foo" path="foo">
    <project name="sub" path="sub" remote="unknown" />
  </project>
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        let err = validate_manifest(&manifest).unwrap_err();
        assert!(
            err.to_string()
                .contains("unknown remote reference: unknown")
        );
    }

    #[test]
    fn parse_remote_with_all_optional_attrs() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" alias="o" fetch="https://example.com" pushurl="https://push.example.com" review="https://review.example.com" revision="main" />
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        let remote = &manifest.remotes["origin"];
        assert_eq!(remote.alias, Some("o".to_string()));
        assert_eq!(remote.pushurl, Some("https://push.example.com".to_string()));
        assert_eq!(
            remote.review,
            Some("https://review.example.com".to_string())
        );
        assert_eq!(remote.revision, Some("main".to_string()));
    }

    #[test]
    fn parse_project_with_empty_groups() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://example.com" />
  <default remote="origin" revision="main" />
  <project name="foo" path="foo" groups="" />
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert!(manifest.projects[0].groups.is_empty());
    }

    #[test]
    fn parse_project_with_no_path() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://example.com" />
  <default remote="origin" revision="main" />
  <project name="foo" />
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert_eq!(manifest.projects[0].path, None);
    }

    #[test]
    fn parse_default_with_all_optional_attrs() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://example.com" />
  <default remote="origin" revision="main" dest-branch="main" upstream="main" sync-j="4" sync-j-max="8" sync-c="true" sync-s="false" sync-tags="true" />
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        let default = manifest.default.as_ref().unwrap();
        assert_eq!(default.dest_branch, Some("main".to_string()));
        assert_eq!(default.upstream, Some("main".to_string()));
        assert_eq!(default.sync_j, Some(4));
        assert_eq!(default.sync_j_max, Some(8));
        assert_eq!(default.sync_c, Some(true));
        assert_eq!(default.sync_s, Some(false));
        assert_eq!(default.sync_tags, Some(true));
    }

    #[test]
    fn parse_annotation_keep_false() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://example.com" />
  <default remote="origin" revision="main" />
  <project name="foo" path="foo">
    <annotation name="ann" value="val" keep="false" />
  </project>
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert_eq!(manifest.projects[0].annotations[0].keep, false);
    }

    #[test]
    fn parse_annotation_keep_default() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://example.com" />
  <default remote="origin" revision="main" />
  <project name="foo" path="foo">
    <annotation name="ann" value="val" />
  </project>
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert_eq!(manifest.projects[0].annotations[0].keep, true);
    }

    #[test]
    fn parse_remove_project_minimal() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remove-project name="foo" />
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert_eq!(manifest.remove_projects.len(), 1);
        assert_eq!(manifest.remove_projects[0].name, Some("foo".to_string()));
        assert_eq!(manifest.remove_projects[0].optional, false);
    }

    #[test]
    fn parse_extend_project_minimal() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <extend-project name="foo" />
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert_eq!(manifest.extend_projects.len(), 1);
        assert_eq!(manifest.extend_projects[0].name, "foo");
    }

    #[test]
    fn parse_submanifest_minimal() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <submanifest name="sub" />
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert_eq!(manifest.submanifests.len(), 1);
        assert_eq!(manifest.submanifests["sub"].name, "sub");
    }

    #[test]
    fn parse_include_minimal() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <include name="other.xml" />
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert_eq!(manifest.includes.len(), 1);
        assert_eq!(manifest.includes[0].name, "other.xml");
        assert!(manifest.includes[0].groups.is_empty());
    }

    #[test]
    fn parse_manifest_server_minimal() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <manifest-server url="https://example.com" />
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert_eq!(
            manifest.manifest_server,
            Some("https://example.com".to_string())
        );
    }

    #[test]
    fn parse_contactinfo_minimal() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <contactinfo bugurl="https://bugs.example.com" />
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert_eq!(
            manifest.contactinfo.as_ref().unwrap().bugurl,
            "https://bugs.example.com"
        );
    }

    #[test]
    fn parse_superproject_minimal() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <superproject name="super" />
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert_eq!(manifest.superproject.as_ref().unwrap().name, "super");
    }

    #[test]
    fn parse_repo_hooks_minimal() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <repo-hooks in-project="hooks" enabled-list="list.txt" />
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert_eq!(manifest.repo_hooks.as_ref().unwrap().in_project, "hooks");
        assert_eq!(
            manifest.repo_hooks.as_ref().unwrap().enabled_list,
            "list.txt"
        );
    }

    #[test]
    fn validate_manifest_detects_unknown_remote() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <project name="foo" remote="missing" />
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        let err = validate_manifest(&manifest).unwrap_err();
        assert!(
            err.to_string()
                .contains("unknown remote reference: missing")
        );
    }

    #[test]
    fn validate_manifest_accepts_known_remote() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://example.com" />
  <project name="foo" remote="origin" />
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert!(validate_manifest(&manifest).is_ok());
    }

    #[test]
    fn parse_empty_project_as_subproject() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://example.com" />
  <default remote="origin" revision="main" />
  <project name="parent" path="parent">
    <project name="child" path="child" />
  </project>
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert_eq!(manifest.projects.len(), 1);
        assert_eq!(manifest.projects[0].subprojects.len(), 1);
        assert_eq!(manifest.projects[0].subprojects[0].name, "child");
    }

    #[test]
    fn parse_nested_copyfile_in_project() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://example.com" />
  <default remote="origin" revision="main" />
  <project name="parent" path="parent">
    <copyfile src="a" dest="b" />
    <project name="child" path="child">
      <copyfile src="c" dest="d" />
    </project>
  </project>
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert_eq!(manifest.projects[0].copyfiles.len(), 1);
        assert_eq!(manifest.projects[0].subprojects[0].copyfiles.len(), 1);
    }

    #[test]
    fn parse_nested_linkfile_in_project() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://example.com" />
  <default remote="origin" revision="main" />
  <project name="parent" path="parent">
    <linkfile src="a" dest="b" />
    <project name="child" path="child">
      <linkfile src="c" dest="d" />
    </project>
  </project>
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert_eq!(manifest.projects[0].linkfiles.len(), 1);
        assert_eq!(manifest.projects[0].subprojects[0].linkfiles.len(), 1);
    }

    #[test]
    fn parse_nested_annotation_in_project() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://example.com" />
  <default remote="origin" revision="main" />
  <project name="parent" path="parent">
    <annotation name="a" value="1" />
    <project name="child" path="child">
      <annotation name="b" value="2" />
    </project>
  </project>
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert_eq!(manifest.projects[0].annotations.len(), 1);
        assert_eq!(manifest.projects[0].annotations[0].name, "a");
        assert_eq!(manifest.projects[0].subprojects[0].annotations.len(), 1);
        assert_eq!(manifest.projects[0].subprojects[0].annotations[0].name, "b");
    }

    #[test]
    fn parse_elements_as_start_tags() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://example.com"></remote>
  <default remote="origin" revision="main"></default>
  <remove-project name="foo"></remove-project>
  <submanifest name="sub"></submanifest>
  <repo-hooks in-project="hooks" enabled-list="list.txt"></repo-hooks>
  <superproject name="super"></superproject>
  <contactinfo bugurl="https://bugs.example.com"></contactinfo>
  <manifest-server url="https://example.com"></manifest-server>
  <include name="other.xml"></include>
  <project name="foo" path="foo">
    <annotation name="ann" value="val"></annotation>
    <copyfile src="a" dest="b"></copyfile>
    <linkfile src="c" dest="d"></linkfile>
  </project>
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert_eq!(manifest.remotes.len(), 1);
        assert!(manifest.default.is_some());
        assert_eq!(manifest.remove_projects.len(), 1);
        assert_eq!(manifest.submanifests.len(), 1);
        assert!(manifest.repo_hooks.is_some());
        assert!(manifest.superproject.is_some());
        assert!(manifest.contactinfo.is_some());
        assert!(manifest.manifest_server.is_some());
        assert_eq!(manifest.includes.len(), 1);
        assert_eq!(manifest.projects[0].annotations.len(), 1);
        assert_eq!(manifest.projects[0].copyfiles.len(), 1);
        assert_eq!(manifest.projects[0].linkfiles.len(), 1);
    }

    #[test]
    fn parse_unknown_empty_tag() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <unknown-tag />
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert!(manifest.projects.is_empty());
    }

    #[test]
    fn reject_duplicate_remote_start_tag() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <remote name="origin" fetch="https://example.com"></remote>
  <remote name="origin" fetch="https://other.com"></remote>
</manifest>
"#;
        let err = parse_manifest(xml).unwrap_err();
        assert!(err.to_string().contains("duplicate remote name: origin"));
    }

    #[test]
    fn reject_duplicate_submanifest_start_tag() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <submanifest name="sub"></submanifest>
  <submanifest name="sub"></submanifest>
</manifest>
"#;
        let err = parse_manifest(xml).unwrap_err();
        assert!(err.to_string().contains("duplicate submanifest name: sub"));
    }

    #[test]
    fn reject_malformed_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <unclosed>
</manifest>
"#;
        assert!(parse_manifest(xml).is_err());
    }

    #[test]
    fn parse_notice_with_numeric_char_refs() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <notice>A&#65; B&#x42;</notice>
</manifest>
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert_eq!(manifest.notice, Some("AA BB".to_string()));
    }

    #[test]
    fn parse_notice_with_eof() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest>
  <notice>unclosed text
"#;
        let manifest = parse_manifest(xml).unwrap();
        assert_eq!(manifest.notice, Some("unclosed text\n".to_string()));
    }
}
