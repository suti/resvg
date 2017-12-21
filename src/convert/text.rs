// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use svgdom;
use svgdom::types::{
    FuzzyEq,
};

use dom;

use short::{
    AId,
    EId,
};

use traits::{
    GetValue,
};

use super::{
    fill,
    stroke,
};


pub fn convert(
    defs: &[dom::RefElement],
    text_elem: &svgdom::Node,
) -> Option<dom::Element>
{
    let attrs = text_elem.attributes();
    let ts = attrs.get_transform(AId::Transform).unwrap_or_default();

    if let Some(chunks) = convert_chunks(defs, text_elem) {
        Some(dom::Element {
            id: String::new(),
            kind: dom::ElementKind::Text(dom::Text {
                children: chunks,
            }),
            transform: ts,
        })
    } else {
        None
    }
}

fn convert_chunks(
    defs: &[dom::RefElement],
    text_elem: &svgdom::Node,
) -> Option<Vec<dom::TextChunk>> {
    let mut chunks = Vec::new();
    let mut tspans = Vec::new();

    let ref root_attrs = text_elem.attributes();
    let mut prev_x = resolve_pos(root_attrs, AId::X).unwrap_or(0.0);
    let mut prev_y = resolve_pos(root_attrs, AId::Y).unwrap_or(0.0);

    let mut first_chunk = text_elem.clone();

    for tspan in text_elem.children() {
        debug_assert!(tspan.is_tag_name(EId::Tspan));

        let text = if let Some(node) = tspan.first_child() {
            node.text().clone()
        } else {
            continue;
        };


        let ref attrs = tspan.attributes();
        let x = resolve_pos(attrs, AId::X);
        let y = resolve_pos(attrs, AId::Y);

        if x.is_some() || y.is_some() {
            let tx = x.unwrap_or(0.0);
            let ty = y.unwrap_or(0.0);

            if !tspans.is_empty() {
                if tx.fuzzy_ne(&prev_x) || ty.fuzzy_ne(&prev_y) {
                    chunks.push(create_text_chunk(prev_x, prev_y, &tspans, &first_chunk));
                    tspans.clear();
                }
            }

            prev_x = x.unwrap_or(prev_x);
            prev_y = y.unwrap_or(prev_y);
            first_chunk = tspan.clone();
        }

        tspans.push(dom::TSpan {
            fill: fill::convert(defs, attrs),
            stroke: stroke::convert(defs, attrs),
            font: convert_font(attrs),
            decoration: conv_tspan_decoration2(defs, text_elem, &tspan),
            text: text,
        });
    }

    if !tspans.is_empty() {
        chunks.push(create_text_chunk(prev_x, prev_y, &tspans, &first_chunk));
    }

    Some(chunks)
}

fn resolve_pos(attrs: &svgdom::Attributes, aid: AId) -> Option<f64> {
    if let Some(ref list) = attrs.get_number_list(aid) {
        if !list.is_empty() {
            if list.len() > 1 {
                warn!("List of 'x', 'y' coordinates are not supported in a 'text' element.");
            }

            return Some(list[0]);
        }
    }

    None
}

fn create_text_chunk(
    x: f64,
    y: f64,
    tspans: &[dom::TSpan],
    chunk_node: &svgdom::Node,
) -> dom::TextChunk {
    let ref attrs = chunk_node.attributes();

    dom::TextChunk {
        x,
        y,
        anchor: conv_text_anchor(attrs),
        children: tspans.into(),
    }
}

struct TextDecoTypes {
    has_underline: bool,
    has_overline: bool,
    has_line_through: bool,
}

// 'text-decoration' defined in the 'text' element
// should be generated by 'prepare_text_decoration'.
fn conv_text_decoration(node: &svgdom::Node) -> TextDecoTypes {
    debug_assert!(node.is_tag_name(EId::Text));

    let attrs = node.attributes();

    let def = String::new();
    let text = attrs.get_string(AId::TextDecoration).unwrap_or(&def);

    TextDecoTypes {
        has_underline: text.contains("underline"),
        has_overline: text.contains("overline"),
        has_line_through: text.contains("linethrough"),
    }
}

// 'text-decoration' in 'tspan' does not depend on parent elements.
fn conv_tspan_decoration(tspan: &svgdom::Node) -> TextDecoTypes {
    debug_assert!(tspan.is_tag_name(EId::Tspan));

    let attrs = tspan.attributes();

    let has_attr = |decoration_id: svgdom::ValueId| {
        if let Some(id) = attrs.get_predef(AId::TextDecoration) {
            if id == decoration_id {
                return true;
            }
        }

        false
    };

    TextDecoTypes {
        has_underline: has_attr(svgdom::ValueId::Underline),
        has_overline: has_attr(svgdom::ValueId::Overline),
        has_line_through: has_attr(svgdom::ValueId::LineThrough),
    }
}

fn conv_tspan_decoration2(
    defs: &[dom::RefElement],
    node: &svgdom::Node,
    tspan: &svgdom::Node
) -> dom::TextDecoration {
    let text_dec = conv_text_decoration(node);
    let tspan_dec = conv_tspan_decoration(tspan);

    let gen_style = |in_tspan: bool, in_text: bool| {
        let n = if in_tspan {
            tspan.clone()
        } else if in_text {
            node.clone()
        } else {
            return None;
        };

        let ref attrs = n.attributes();
        let fill = fill::convert(defs, attrs);
        let stroke = stroke::convert(defs, attrs);

        Some(dom::TextDecorationStyle {
            fill,
            stroke,
        })
    };

    dom::TextDecoration {
        underline: gen_style(tspan_dec.has_underline, text_dec.has_underline),
        overline: gen_style(tspan_dec.has_overline, text_dec.has_overline),
        line_through: gen_style(tspan_dec.has_line_through, text_dec.has_line_through),
    }
}

fn conv_text_anchor(attrs: &svgdom::Attributes) -> dom::TextAnchor {
    let av = attrs.get_predef(AId::TextAnchor).unwrap_or(svgdom::ValueId::Start);

    match av {
        svgdom::ValueId::Start => dom::TextAnchor::Start,
        svgdom::ValueId::Middle => dom::TextAnchor::Middle,
        svgdom::ValueId::End => dom::TextAnchor::End,
        _ => dom::TextAnchor::Start,
    }
}

fn convert_font(attrs: &svgdom::Attributes) -> dom::Font {
    let style = attrs.get_predef(AId::FontStyle).unwrap_or(svgdom::ValueId::Normal);
    let style = match style {
        svgdom::ValueId::Normal => dom::FontStyle::Normal,
        svgdom::ValueId::Italic => dom::FontStyle::Italic,
        svgdom::ValueId::Oblique => dom::FontStyle::Oblique,
        _ => dom::FontStyle::Normal,
    };

    let variant = attrs.get_predef(AId::FontVariant).unwrap_or(svgdom::ValueId::Normal);
    let variant = match variant {
        svgdom::ValueId::Normal => dom::FontVariant::Normal,
        svgdom::ValueId::SmallCaps => dom::FontVariant::SmallCaps,
        _ => dom::FontVariant::Normal,
    };

    let weight = attrs.get_predef(AId::FontWeight).unwrap_or(svgdom::ValueId::Normal);
    let weight = match weight {
        svgdom::ValueId::Normal => dom::FontWeight::Normal,
        svgdom::ValueId::Bold => dom::FontWeight::Bold,
        svgdom::ValueId::Bolder => dom::FontWeight::Bolder,
        svgdom::ValueId::Lighter => dom::FontWeight::Lighter,
        svgdom::ValueId::N100 => dom::FontWeight::W100,
        svgdom::ValueId::N200 => dom::FontWeight::W200,
        svgdom::ValueId::N300 => dom::FontWeight::W300,
        svgdom::ValueId::N400 => dom::FontWeight::W400,
        svgdom::ValueId::N500 => dom::FontWeight::W500,
        svgdom::ValueId::N600 => dom::FontWeight::W600,
        svgdom::ValueId::N700 => dom::FontWeight::W700,
        svgdom::ValueId::N800 => dom::FontWeight::W800,
        svgdom::ValueId::N900 => dom::FontWeight::W900,
        _ => dom::FontWeight::Normal,
    };

    let stretch = attrs.get_predef(AId::FontStretch).unwrap_or(svgdom::ValueId::Normal);
    let stretch = match stretch {
        svgdom::ValueId::Normal => dom::FontStretch::Normal,
        svgdom::ValueId::Wider => dom::FontStretch::Wider,
        svgdom::ValueId::Narrower => dom::FontStretch::Narrower,
        svgdom::ValueId::UltraCondensed => dom::FontStretch::UltraCondensed,
        svgdom::ValueId::ExtraCondensed => dom::FontStretch::ExtraCondensed,
        svgdom::ValueId::Condensed => dom::FontStretch::Condensed,
        svgdom::ValueId::SemiCondensed => dom::FontStretch::SemiCondensed,
        svgdom::ValueId::SemiExpanded => dom::FontStretch::SemiExpanded,
        svgdom::ValueId::Expanded => dom::FontStretch::Expanded,
        svgdom::ValueId::ExtraExpanded => dom::FontStretch::ExtraExpanded,
        svgdom::ValueId::UltraExpanded => dom::FontStretch::UltraExpanded,
        _ => dom::FontStretch::Normal,
    };

    // TODO: remove text nodes with font-size <= 0
    let size = attrs.get_number(AId::FontSize).unwrap_or(::DEFAULT_FONT_SIZE);
    debug_assert!(size > 0.0);

    let family = attrs.get_string(AId::FontFamily)
                      .unwrap_or(&::DEFAULT_FONT_FAMILY.to_owned()).clone();

    dom::Font {
        family,
        size,
        style,
        variant,
        weight,
        stretch,
    }
}
