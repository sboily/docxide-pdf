//! Laid-out pages, independent of PDF: what each page of a converted document
//! draws, in painting order, with the fonts and pictures it draws them with.
//!
//! [`crate::layout_docx_bytes_with`] returns a [`DisplayDocument`]: the same
//! layout [`crate::convert_docx_bytes_to_pdf_bytes_with`] writes as PDF, for a
//! caller that writes or renders pages itself. Coordinates are PDF user space:
//! points, origin at the page's bottom-left corner, y up.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

/// A 2-D affine matrix `[a, b, c, d, e, f]`, as PDF writes it:
/// `x' = a·x + c·y + e`, `y' = b·x + d·y + f`.
pub type Matrix = [f32; 6];

/// The whole document, laid out.
#[derive(Debug, Clone, Default)]
pub struct DisplayDocument {
    pub pages: Vec<Page>,
    /// The fonts [`TextRun::font`] indexes.
    pub fonts: Vec<Font>,
    /// The pictures [`Item::Image`] indexes.
    pub images: Vec<Image>,
    pub info: Info,
    /// The logical structure (tagged PDF) tying the pages' marked content
    /// ([`Item::BeginMarked`]) and links to what they are, when the document
    /// is tagged.
    pub structure: Option<Structure>,
    /// The bookmarks, in reading order: one per heading.
    pub outline: Vec<OutlineEntry>,
    /// What the layout drew that this description does not carry yet, by
    /// kind, with how many times: a writer of these pages draws less than
    /// the PDF export where this is not empty.
    pub unsupported: BTreeMap<&'static str, usize>,
}

/// What the document says about itself.
#[derive(Debug, Clone, Default)]
pub struct Info {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Option<String>,
    /// BCP 47 language tag of the document's text.
    pub language: Option<String>,
}

/// One page.
#[derive(Debug, Clone, Default)]
pub struct Page {
    pub width: f32,
    pub height: f32,
    pub items: Vec<Item>,
    pub links: Vec<Link>,
}

/// What a page draws. Items are painted in order; a [`Item::Transform`] or
/// [`Item::Clip`] applies to the items after it, up to the end of the
/// enclosing [`Item::Group`] (or page).
#[derive(Debug, Clone)]
pub enum Item {
    /// Items drawn in a state of their own: transforms and clips inside end
    /// with the group.
    Group(Vec<Item>),
    /// Concatenated to the current transform.
    Transform(Matrix),
    /// Intersected with the current clip.
    Clip {
        path: Path,
        fill_rule: FillRule,
    },
    Fill {
        path: Path,
        fill_rule: FillRule,
        paint: Paint,
    },
    Stroke {
        path: Path,
        paint: Paint,
        style: StrokeStyle,
    },
    /// The path filled with a gradient.
    FillGradient {
        path: Path,
        fill_rule: FillRule,
        gradient: LinearGradient,
    },
    Text(TextRun),
    /// A picture of [`DisplayDocument::images`], drawn into the unit square
    /// of the current transform (as a PDF image XObject is).
    Image(usize),
    /// What is drawn from here to the matching [`Item::EndMarked`] is marked:
    /// content of a structure element, or an artifact. Marked sequences nest,
    /// and may begin and end in different groups, as PDF's do.
    BeginMarked(Mark),
    EndMarked,
}

/// What a marked sequence of a page is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mark {
    /// Content of a structure element of type `tag`, which lists it as
    /// [`Kid::Content`] by `mcid`, unique on its page.
    Element { tag: String, mcid: u32 },
    /// Not part of the document's content: borders, shading, the comment
    /// pane, or headers and footers (`pagination`).
    Artifact { pagination: bool },
}

/// The document's logical structure: its elements, the first the root
/// (`Document`), each listing what it holds in reading order.
#[derive(Debug, Clone, Default)]
pub struct Structure {
    pub elements: Vec<Element>,
}

/// An element of the structure.
#[derive(Debug, Clone, Default)]
pub struct Element {
    /// A standard structure type: `Document`, `P`, `H1`, `L`, `LI`, `Lbl`,
    /// `LBody`, `Table`, `TR`, `TH`, `TD`, `Figure`, `Formula`, `Note`,
    /// `Span`, `Link`...
    pub kind: String,
    pub kids: Vec<Kid>,
    /// Alternate text (a figure's, a formula's).
    pub alt: Option<String>,
    /// The text the element's content stands for (letters drawn as small
    /// caps, for one).
    pub actual_text: Option<String>,
    /// BCP 47 tag of the element's language, when not the document's.
    pub lang: Option<String>,
    /// The element's identifier (each note has one).
    pub id: Option<String>,
    pub table_cell: Option<TableCell>,
    pub list_numbering: Option<ListNumbering>,
}

/// What an element holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kid {
    /// An element of [`Structure::elements`], by index.
    Element(usize),
    /// The page's sequence marked [`Mark::Element`] with this `mcid`.
    Content { page: usize, mcid: u32 },
    /// A link area: the index in its page's [`Page::links`].
    Link { page: usize, link: usize },
}

/// A table cell's attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableCell {
    /// For a header cell, the cells it heads.
    pub header_scope: Option<HeaderScope>,
    /// Columns the cell spans, 1 or more.
    pub col_span: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderScope {
    Row,
    Column,
    Both,
}

/// How a list's items are numbered, as PDF names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListNumbering {
    Unordered,
    Ordered,
    Description,
    Disc,
    Circle,
    Square,
    Decimal,
    LowerRoman,
    UpperRoman,
    LowerAlpha,
    UpperAlpha,
}

/// A bookmark: a heading and where it is.
#[derive(Debug, Clone, PartialEq)]
pub struct OutlineEntry {
    /// The heading's level: an entry nests under the nearest entry before it
    /// of a smaller level.
    pub level: u8,
    pub title: String,
    pub page: usize,
    /// The heading's top, in the page's user space.
    pub y: f32,
}

/// A path in the current user space.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Path(pub Vec<Segment>);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Segment {
    MoveTo(f32, f32),
    LineTo(f32, f32),
    CubicTo(f32, f32, f32, f32, f32, f32),
    /// A closed rectangle: corner, width, height.
    Rect(f32, f32, f32, f32),
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}

/// A colour and its opacity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Paint {
    pub color: Color,
    /// 0 (transparent) to 1 (opaque).
    pub alpha: f32,
}

/// Device colours, components from 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Color {
    Gray(f32),
    Rgb(f32, f32, f32),
}

/// A linear (axial) gradient. Its axis is in the page's default user space, as a PDF pattern's
/// is: the transforms in force where it fills do not move it.
#[derive(Debug, Clone, PartialEq)]
pub struct LinearGradient {
    pub start: [f32; 2],
    pub end: [f32; 2],
    /// Offsets from 0 (`start`) to 1 (`end`), ascending.
    pub stops: Vec<GradientStop>,
    /// Whether the first and last colours go on before `start` and past `end`.
    pub extend: [bool; 2],
    /// Interpolated in linear light, as Word draws gradients, rather than between sRGB values.
    pub linear_light: bool,
}

/// A colour of a gradient, sRGB components from 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradientStop {
    pub offset: f32,
    pub color: [f32; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct StrokeStyle {
    pub width: f32,
    pub cap: LineCap,
    pub join: LineJoin,
    pub miter_limit: f32,
    /// Dash lengths and phase; empty lengths draw a solid line.
    pub dash: (Vec<f32>, f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

/// Glyphs of one font on one baseline.
#[derive(Debug, Clone)]
pub struct TextRun {
    /// Index into [`DisplayDocument::fonts`].
    pub font: usize,
    /// Font size, in text space units.
    pub size: f32,
    /// Text space to the current user space at the run's origin. Synthetic
    /// italics are a shear in it.
    pub matrix: Matrix,
    /// Horizontal scaling of the glyphs, 1 = 100%.
    pub horizontal_scaling: f32,
    pub mode: TextMode,
    pub fill: Paint,
    /// Paint and line width of the outline, for [`TextMode::Stroke`] and
    /// [`TextMode::FillStroke`] (synthetic bold).
    pub stroke: Option<(Paint, f32)>,
    pub glyphs: Vec<Glyph>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextMode {
    Fill,
    Stroke,
    FillStroke,
    /// Drawn for its text only: searchable, not visible.
    Invisible,
}

/// One glyph of a [`TextRun`].
#[derive(Debug, Clone, PartialEq)]
pub struct Glyph {
    /// The glyph id in the font file; for a [`FontSource::Standard`] font,
    /// its WinAnsi code.
    pub id: u16,
    /// Where the glyph's origin is along the baseline, in text space units
    /// from the run's origin (horizontal scaling included).
    pub x: f32,
    /// How far the glyph moves the pen, in thousandths of the font size
    /// (before horizontal scaling): the font's advance as the PDF export
    /// rounds it.
    pub advance: f32,
    /// The text the glyph stands for (several characters for a ligature,
    /// none for a glyph that stands for nothing).
    pub text: String,
}

/// A font, as the layout measured it.
#[derive(Debug, Clone)]
pub struct Font {
    pub source: FontSource,
    /// The PostScript-like name the PDF export gives it.
    pub name: String,
}

#[derive(Debug, Clone)]
pub enum FontSource {
    /// A font file on disk; `index` is the face in a collection.
    File { path: PathBuf, index: u32 },
    /// A font program the document embeds.
    Data { data: Arc<Vec<u8>>, index: u32 },
    /// One of the PDF standard 14 fonts, by its PostScript name, drawn with
    /// WinAnsi codes.
    Standard(&'static str),
}

/// A picture, encoded as the PDF export encodes it.
#[derive(Debug, Clone)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub color_space: ImageColorSpace,
    pub bits_per_component: u8,
    pub filter: ImageFilter,
    /// The samples, encoded by `filter`.
    pub data: Vec<u8>,
    /// Opacity per pixel, a DeviceGray picture.
    pub soft_mask: Option<Box<Image>>,
    pub interpolate: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ImageColorSpace {
    Gray,
    Rgb,
    Cmyk,
    /// A palette of RGB triples, `palette.len() / 3` entries.
    IndexedRgb {
        palette: Vec<u8>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFilter {
    /// JPEG (DCTDecode).
    Jpeg,
    /// zlib (FlateDecode).
    Flate,
    /// Raw samples.
    None,
}

/// A link area on a page.
#[derive(Debug, Clone)]
pub struct Link {
    /// Left, bottom, right, top.
    pub rect: [f32; 4],
    pub target: LinkTarget,
    /// The linked text, a description of the link.
    pub text: String,
}

#[derive(Debug, Clone)]
pub enum LinkTarget {
    Uri(String),
    /// A page index and the y to show at the top.
    Page {
        page: usize,
        y: f32,
    },
}
