use base_db::{EditionedFileId, FilePosition, FileRange, RangeInfo, SourceDatabase as _};
use hir::{Semantics, definition::Definition};
use hir_ty::ty::pretty::pretty_fn;
use ide_db::RootDatabase;
use syntax::{AstNode as _, SyntaxKind};
use wgsl_types::builtin::{BuiltinSignature, builtin_fn_signatures};

use crate::{NavigationTarget, helpers, markup::Markup};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HoverConfig {
    pub links_in_hover: bool,
    pub memory_layout: Option<MemoryLayoutHoverConfig>,
    pub documentation: bool,
    pub keywords: bool,
    pub format: HoverDocFormat,
    pub max_fields_count: Option<usize>,
    pub max_enum_variants_count: Option<usize>,
    pub max_substitution_type_length: SubstitutionTypeLength,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubstitutionTypeLength {
    Unlimited,
    LimitTo(usize),
    Hide,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MemoryLayoutHoverConfig {
    pub size: Option<MemoryLayoutHoverRenderKind>,
    pub offset: Option<MemoryLayoutHoverRenderKind>,
    pub alignment: Option<MemoryLayoutHoverRenderKind>,
    pub padding: Option<MemoryLayoutHoverRenderKind>,
    pub niches: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MemoryLayoutHoverRenderKind {
    Decimal,
    Hexadecimal,
    Both,
}

/// Contains the results when hovering over an item.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq)]
pub struct HoverResult {
    pub markup: Markup,
    pub actions: Vec<HoverAction>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HoverDocFormat {
    Markdown,
    PlainText,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum HoverAction {
    Implementation(FilePosition),
    Reference(FilePosition),
    GoToType(Vec<HoverGotoTypeData>),
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct HoverGotoTypeData {
    pub mod_path: String,
    pub navigation_target: NavigationTarget,
}

// Feature: Hover
//
// Shows additional information, like the type of an expression or the documentation for a definition when "focusing" code.
// Focusing is usually hovering with a mouse, but can also be triggered with a shortcut.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "infeasible to list all syntax kinds"
)]
pub(crate) fn hover(
    db: &RootDatabase,
    file_range: FileRange,
    config: &HoverConfig,
) -> Option<RangeInfo<HoverResult>> {
    let semantics = &Semantics::new(db);
    let file_id = EditionedFileId::from_file(db, file_range.file_id);
    let file = file_id.parse(db).tree();
    let token = helpers::pick_best_token(
        file.syntax().token_at_offset(file_range.range.start()),
        |token| match token {
            SyntaxKind::Identifier => 2,
            kind if kind.is_trivia() => 0,
            _ => 1,
        },
    )?;

    let definition = Definition::from_token(semantics, file_id, &token)?;
    let declarations = match definition {
        Definition::BuiltinFunction(name) => {
            let signatures = builtin_fn_signatures(name.as_str());
            if signatures.is_empty() {
                return None;
            }
            signatures
                .iter()
                .copied()
                .map(|signature| render_builtin_signature(name.as_str(), signature))
                .collect::<Vec<_>>()
                .join("\n")
        },
        Definition::ModuleDef(hir::ModuleDef::Function(function)) => {
            pretty_fn(db, function.details(db))
        },
        _ => return None,
    };
    let markup = match config.format {
        HoverDocFormat::Markdown => format!("```wgsl\n{declarations}\n```").into(),
        HoverDocFormat::PlainText => declarations.into(),
    };

    Some(RangeInfo::new(
        token.text_range(),
        HoverResult {
            markup,
            actions: Vec::new(),
        },
    ))
}

fn render_builtin_signature(
    name: &str,
    signature: BuiltinSignature,
) -> String {
    let mut result = String::from("fn ");
    result.push_str(name);

    if !signature.template_parameters.is_empty() {
        result.push('<');
        result.push_str(&signature.template_parameters.join(", "));
        result.push('>');
    }

    result.push('(');
    for (index, parameter) in signature.parameters.iter().enumerate() {
        if index != 0 {
            result.push_str(", ");
        }
        result.push_str(parameter.name);
        result.push_str(": ");
        result.push_str(parameter.ty);
    }
    result.push(')');

    if let Some(return_type) = signature.return_type {
        result.push_str(" -> ");
        result.push_str(return_type);
    }

    result
}

#[cfg(test)]
mod tests {
    use base_db::{FileRange, TextRange};
    use test_utils::extract_offset;

    use super::*;
    use crate::Analysis;

    fn hover_config(format: HoverDocFormat) -> HoverConfig {
        HoverConfig {
            links_in_hover: false,
            memory_layout: None,
            documentation: false,
            keywords: false,
            format,
            max_fields_count: None,
            max_enum_variants_count: None,
            max_substitution_type_length: SubstitutionTypeLength::Unlimited,
        }
    }

    fn builtin_hover(
        source: &str,
        format: HoverDocFormat,
    ) -> String {
        let (offset, source) = extract_offset(source);
        let (analysis, file_id) = Analysis::from_single_file(source);
        let result = analysis
            .hover(
                &hover_config(format),
                FileRange {
                    file_id,
                    range: TextRange::empty(offset),
                },
            )
            .unwrap()
            .unwrap();
        result.info.markup.to_string()
    }

    #[test]
    fn builtin_select_hover() {
        let hover = builtin_hover(
            r#"
fn main() {
    let value = sele$0ct(1, 2, true);
}
"#,
            HoverDocFormat::PlainText,
        );

        assert_eq!(
            hover,
            "fn select(f: T, t: T, cond: bool) -> T\n\
             fn select(f: vecN<T>, t: vecN<T>, cond: vecN<bool>) -> vecN<T>"
        );
    }

    #[test]
    fn builtin_texture_store_hover_uses_wgsl_markdown() {
        let hover = builtin_hover(
            r#"
fn main() {
    textureSt$0ore(texture, vec2(0), vec4(0.0));
}
"#,
            HoverDocFormat::Markdown,
        );

        assert!(hover.starts_with("```wgsl\nfn textureStore("));
        assert!(hover.contains("texture_storage_2d_array<F, AM>"));
        assert_eq!(hover.matches("fn textureStore(").count(), 4);
        assert!(hover.ends_with("\n```"));
    }

    #[test]
    fn user_function_hover() {
        let hover = builtin_hover(
            r#"
fn add(left: i32, right: i32) -> i32 {
    return left + right;
}

fn main() {
    let value = ad$0d(1, 2);
}
"#,
            HoverDocFormat::Markdown,
        );

        assert_eq!(hover, "```wgsl\nfn add(left: i32, right: i32) -> i32\n```");
    }

    #[test]
    fn user_function_declaration_hover() {
        let hover = builtin_hover(
            r#"
fn ad$0d(left: i32, right: i32) -> i32 {
    return left + right;
}
"#,
            HoverDocFormat::PlainText,
        );

        assert_eq!(hover, "fn add(left: i32, right: i32) -> i32");
    }
}
