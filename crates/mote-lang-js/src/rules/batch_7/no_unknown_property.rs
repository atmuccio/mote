//! `js/no-unknown-property`

use crate::prelude::*;
use crate::rules::batch_6::util::{import_sources, uses_global};

pub struct NoUnknownProperty;

pub static NO_UNKNOWN_PROPERTY: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["react", "suspicious"],
    remediation_minutes: 1,
    description: r#"Flags HTML-style attribute names on built-in elements in React JSX: `class`, `for`, lowercase event
handlers such as `onclick`, and lowercase or dashed spellings such as `tabindex`, `maxlength` or
`stroke-width`, where React expects `className`, `htmlFor`, `onClick`, `tabIndex`, `maxLength`,
`strokeWidth`.

**Why:** React maps its own camel-cased prop names to the DOM. An event handler under the HTML
name (`onclick={handle}`) is never attached — the click silently does nothing. The other HTML
spellings are passed through with a warning in the console, work differently from the React name
in places (`class` is not reflected in `className`, `for` in `htmlFor`), and break type checking.

**Fix:** use the React name.

```jsx
<label class="field" for="name">Name</label>     // flagged twice
<button onclick={save}>Save</button>            // flagged: never called
<input tabindex="1" maxlength={20} />           // flagged twice
<label className="field" htmlFor="name">Name</label>
<button onClick={save}>Save</button>            // compliant
```

Only files that import React (or `react-dom`, `next`) and no other JSX library are checked
(Preact, Solid, Vue, Qwik and others accept the HTML names). Components (`<Button class=…>`),
custom elements (`<my-widget class=…>`), `data-*` and `aria-*` attributes, and names outside a
fixed table of known misspellings are not reported.
"#,
    ..RuleMeta::new(
        "js/no-unknown-property",
        "js",
        "JSX attributes on DOM elements should use React's prop names",
    )
};

/// HTML (or SVG) attribute name → React prop name.
const RENAMES: &[(&str, &str)] = &[
    ("class", "className"),
    ("for", "htmlFor"),
    ("accept-charset", "acceptCharset"),
    ("accesskey", "accessKey"),
    ("allowfullscreen", "allowFullScreen"),
    ("autocapitalize", "autoCapitalize"),
    ("autocomplete", "autoComplete"),
    ("autofocus", "autoFocus"),
    ("autoplay", "autoPlay"),
    ("cellpadding", "cellPadding"),
    ("cellspacing", "cellSpacing"),
    ("charset", "charSet"),
    ("colspan", "colSpan"),
    ("contenteditable", "contentEditable"),
    ("contextmenu", "contextMenu"),
    ("crossorigin", "crossOrigin"),
    ("datetime", "dateTime"),
    ("enctype", "encType"),
    ("enterkeyhint", "enterKeyHint"),
    ("formaction", "formAction"),
    ("formenctype", "formEncType"),
    ("formmethod", "formMethod"),
    ("formnovalidate", "formNoValidate"),
    ("formtarget", "formTarget"),
    ("frameborder", "frameBorder"),
    ("hreflang", "hrefLang"),
    ("http-equiv", "httpEquiv"),
    ("inputmode", "inputMode"),
    ("marginheight", "marginHeight"),
    ("marginwidth", "marginWidth"),
    ("maxlength", "maxLength"),
    ("minlength", "minLength"),
    ("novalidate", "noValidate"),
    ("playsinline", "playsInline"),
    ("readonly", "readOnly"),
    ("referrerpolicy", "referrerPolicy"),
    ("rowspan", "rowSpan"),
    ("spellcheck", "spellCheck"),
    ("srcdoc", "srcDoc"),
    ("srclang", "srcLang"),
    ("srcset", "srcSet"),
    ("tabindex", "tabIndex"),
    ("usemap", "useMap"),
    // SVG presentation attributes.
    ("clip-path", "clipPath"),
    ("clip-rule", "clipRule"),
    ("fill-opacity", "fillOpacity"),
    ("fill-rule", "fillRule"),
    ("font-family", "fontFamily"),
    ("font-size", "fontSize"),
    ("font-weight", "fontWeight"),
    ("stop-color", "stopColor"),
    ("stop-opacity", "stopOpacity"),
    ("stroke-dasharray", "strokeDasharray"),
    ("stroke-dashoffset", "strokeDashoffset"),
    ("stroke-linecap", "strokeLinecap"),
    ("stroke-linejoin", "strokeLinejoin"),
    ("stroke-miterlimit", "strokeMiterlimit"),
    ("stroke-opacity", "strokeOpacity"),
    ("stroke-width", "strokeWidth"),
    ("text-anchor", "textAnchor"),
    ("dominant-baseline", "dominantBaseline"),
    ("viewbox", "viewBox"),
];

/// React's DOM event props, without `on`.
const EVENTS: &[&str] = &[
    "Abort",
    "AnimationEnd",
    "AnimationIteration",
    "AnimationStart",
    "BeforeInput",
    "Blur",
    "CanPlay",
    "CanPlayThrough",
    "Change",
    "Click",
    "CompositionEnd",
    "CompositionStart",
    "CompositionUpdate",
    "ContextMenu",
    "Copy",
    "Cut",
    "DoubleClick",
    "Drag",
    "DragEnd",
    "DragEnter",
    "DragExit",
    "DragLeave",
    "DragOver",
    "DragStart",
    "Drop",
    "DurationChange",
    "Emptied",
    "Ended",
    "Error",
    "Focus",
    "Input",
    "Invalid",
    "KeyDown",
    "KeyPress",
    "KeyUp",
    "Load",
    "LoadedData",
    "LoadedMetadata",
    "LoadStart",
    "MouseDown",
    "MouseEnter",
    "MouseLeave",
    "MouseMove",
    "MouseOut",
    "MouseOver",
    "MouseUp",
    "Paste",
    "Pause",
    "Play",
    "Playing",
    "PointerCancel",
    "PointerDown",
    "PointerEnter",
    "PointerLeave",
    "PointerMove",
    "PointerOut",
    "PointerOver",
    "PointerUp",
    "Progress",
    "RateChange",
    "Reset",
    "Resize",
    "Scroll",
    "Seeked",
    "Seeking",
    "Select",
    "Stalled",
    "Submit",
    "Suspend",
    "TimeUpdate",
    "Toggle",
    "TouchCancel",
    "TouchEnd",
    "TouchMove",
    "TouchStart",
    "TransitionEnd",
    "VolumeChange",
    "Waiting",
    "Wheel",
];

impl JsRule for NoUnknownProperty {
    fn meta(&self) -> &'static RuleMeta {
        &NO_UNKNOWN_PROPERTY
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::JSXOpeningElement]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_jsx() && ctx.file_contains(AstType::JSXOpeningElement) && is_react_only(ctx)
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::JSXOpeningElement(element) = node.kind() else {
            return;
        };
        // Built-in elements only: a lowercase name without a dash (custom elements take any
        // attribute).
        let JSXElementName::Identifier(tag) = &element.name else {
            return;
        };
        let tag = tag.name.as_str();
        if !tag.starts_with(|c: char| c.is_ascii_lowercase()) || tag.contains('-') {
            return;
        }
        // `<button is="fancy-button">` is a customized built-in element.
        let is_customized = element.attributes.iter().any(|a| {
            matches!(a, JSXAttributeItem::Attribute(attr)
                if matches!(&attr.name, JSXAttributeName::Identifier(n) if n.name == "is"))
        });
        if is_customized {
            return;
        }
        for item in &element.attributes {
            let JSXAttributeItem::Attribute(attr) = item else {
                continue;
            };
            let JSXAttributeName::Identifier(name) = &attr.name else {
                continue;
            };
            let name_str = name.name.as_str();
            let Some(fix) = react_name(name_str) else {
                continue;
            };
            let message = if name_str.starts_with("on") && fix.starts_with("on") {
                format!("React never attaches the handler '{name_str}': write it '{fix}'")
            } else {
                format!("Use React's prop name '{fix}' instead of '{name_str}'")
            };
            ctx.emit(name.span, message);
        }
    }
}

/// The React name for an HTML-style attribute name, if it is a known misspelling.
fn react_name(name: &str) -> Option<String> {
    if let Some((_, fix)) = RENAMES.iter().find(|(html, _)| *html == name) {
        return Some((*fix).to_string());
    }
    // `onclick` → `onClick`: the all-lowercase spelling of a React event prop.
    let event = name.strip_prefix("on")?;
    if event.is_empty() || !event.bytes().all(|b| b.is_ascii_lowercase()) {
        return None;
    }
    let event = if event == "dblclick" {
        "doubleclick"
    } else {
        event
    };
    EVENTS
        .iter()
        .find(|camel| camel.eq_ignore_ascii_case(event))
        .map(|camel| format!("on{camel}"))
}

/// The file uses React (an import from `react`, `react-dom`, `next`, or a global `React`) and no
/// other JSX library that accepts HTML attribute names.
fn is_react_only(ctx: &Ctx<'_>) -> bool {
    let mut react = false;
    for source in import_sources(ctx) {
        let package = source.split('/').next().unwrap_or(source);
        match package {
            "react" | "react-dom" | "next" => react = true,
            "preact" | "solid-js" | "vue" | "@vue" | "@builder.io" | "@stencil" | "hyperapp"
            | "inferno" | "mithril" => return false,
            _ => {}
        }
        if source.starts_with("preact") || source.contains("solid") {
            return false;
        }
    }
    react || uses_global(ctx, "React")
}
