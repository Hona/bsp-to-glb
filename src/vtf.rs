use serde::Serialize;
use std::fmt;

const MAX_VTF_BYTES: usize = 256 * 1024 * 1024;
const MAX_DECODED_BYTES: usize = 256 * 1024 * 1024;
const MAX_VTF_DIMENSION: u32 = 16_384;
const MAX_RESOURCES: usize = 32;
const TEXTUREFLAGS_ENVMAP: u32 = 0x0000_4000;
const RESOURCE_NO_DATA_CHUNK: u8 = 0x02;
const LOW_RES_IMAGE_RESOURCE: [u8; 3] = [0x01, 0, 0];
const HIGH_RES_IMAGE_RESOURCE: [u8; 3] = [0x30, 0, 0];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VtfImageSelection {
    pub mip: u8,
    pub frame: u16,
    pub face: u8,
    pub slice: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum VtfErrorKind {
    Invalid,
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VtfError {
    pub kind: VtfErrorKind,
    pub message: String,
}

impl VtfError {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            kind: VtfErrorKind::Invalid,
            message: message.into(),
        }
    }

    fn unsupported(message: impl Into<String>) -> Self {
        Self {
            kind: VtfErrorKind::Unsupported,
            message: message.into(),
        }
    }
}

impl fmt::Display for VtfError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for VtfError {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VtfFormatMetadata {
    pub code: u32,
    pub name: String,
    pub supported: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VtfResourceMetadata {
    pub tag: String,
    pub flags: u8,
    #[serde(rename = "inline")]
    pub is_inline: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline_data: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_length: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VtfMetadata {
    pub version_major: u32,
    pub version_minor: u32,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub frames: u16,
    pub start_frame: u16,
    pub faces: u8,
    pub mip_count: u8,
    pub flags: u32,
    pub format: VtfFormatMetadata,
    pub resources: Vec<VtfResourceMetadata>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedVtf {
    pub metadata: VtfMetadata,
    pub selection: VtfImageSelection,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ImageFormat {
    Rgba8888,
    Abgr8888,
    Rgb888,
    Bgr888,
    Rgb565,
    I8,
    Ia88,
    P8,
    A8,
    Rgb888BlueScreen,
    Bgr888BlueScreen,
    Argb8888,
    Bgra8888,
    Dxt1,
    Dxt3,
    Dxt5,
    Bgrx8888,
    Bgr565,
    Bgrx5551,
    Bgra4444,
    Dxt1OneBitAlpha,
    Bgra5551,
    Uv88,
    Uvwq8888,
    Rgba16161616F,
    Rgba16161616,
    Uvlx8888,
    R32F,
    Rgb323232F,
    Rgba32323232F,
    Ati2N,
    Ati1N,
    NvDst16,
    NvDst24,
    NvIntz,
    NvRawz,
    AtiDst16,
    AtiDst24,
    NvNull,
    Unknown(u32),
}

impl ImageFormat {
    fn from_code(code: u32) -> Self {
        match code {
            0 => Self::Rgba8888,
            1 => Self::Abgr8888,
            2 => Self::Rgb888,
            3 => Self::Bgr888,
            4 => Self::Rgb565,
            5 => Self::I8,
            6 => Self::Ia88,
            7 => Self::P8,
            8 => Self::A8,
            9 => Self::Rgb888BlueScreen,
            10 => Self::Bgr888BlueScreen,
            11 => Self::Argb8888,
            12 => Self::Bgra8888,
            13 => Self::Dxt1,
            14 => Self::Dxt3,
            15 => Self::Dxt5,
            16 => Self::Bgrx8888,
            17 => Self::Bgr565,
            18 => Self::Bgrx5551,
            19 => Self::Bgra4444,
            20 => Self::Dxt1OneBitAlpha,
            21 => Self::Bgra5551,
            22 => Self::Uv88,
            23 => Self::Uvwq8888,
            24 => Self::Rgba16161616F,
            25 => Self::Rgba16161616,
            26 => Self::Uvlx8888,
            27 => Self::R32F,
            28 => Self::Rgb323232F,
            29 => Self::Rgba32323232F,
            30 => Self::NvDst16,
            31 => Self::NvDst24,
            32 => Self::NvIntz,
            33 => Self::NvRawz,
            34 => Self::AtiDst16,
            35 => Self::AtiDst24,
            36 => Self::NvNull,
            37 => Self::Ati2N,
            38 => Self::Ati1N,
            _ => Self::Unknown(code),
        }
    }

    fn name(self) -> String {
        match self {
            Self::Rgba8888 => "RGBA8888",
            Self::Abgr8888 => "ABGR8888",
            Self::Rgb888 => "RGB888",
            Self::Bgr888 => "BGR888",
            Self::Rgb565 => "RGB565",
            Self::I8 => "I8",
            Self::Ia88 => "IA88",
            Self::P8 => "P8",
            Self::A8 => "A8",
            Self::Rgb888BlueScreen => "RGB888_BLUESCREEN",
            Self::Bgr888BlueScreen => "BGR888_BLUESCREEN",
            Self::Argb8888 => "ARGB8888",
            Self::Bgra8888 => "BGRA8888",
            Self::Dxt1 => "DXT1",
            Self::Dxt3 => "DXT3",
            Self::Dxt5 => "DXT5",
            Self::Bgrx8888 => "BGRX8888",
            Self::Bgr565 => "BGR565",
            Self::Bgrx5551 => "BGRX5551",
            Self::Bgra4444 => "BGRA4444",
            Self::Dxt1OneBitAlpha => "DXT1_ONEBITALPHA",
            Self::Bgra5551 => "BGRA5551",
            Self::Uv88 => "UV88",
            Self::Uvwq8888 => "UVWQ8888",
            Self::Rgba16161616F => "RGBA16161616F",
            Self::Rgba16161616 => "RGBA16161616",
            Self::Uvlx8888 => "UVLX8888",
            Self::R32F => "R32F",
            Self::Rgb323232F => "RGB323232F",
            Self::Rgba32323232F => "RGBA32323232F",
            Self::NvDst16 => "NV_DST16",
            Self::NvDst24 => "NV_DST24",
            Self::NvIntz => "NV_INTZ",
            Self::NvRawz => "NV_RAWZ",
            Self::AtiDst16 => "ATI_DST16",
            Self::AtiDst24 => "ATI_DST24",
            Self::NvNull => "NV_NULL",
            Self::Ati2N => "ATI2N",
            Self::Ati1N => "ATI1N",
            Self::Unknown(code) => return format!("UNKNOWN_{code}"),
        }
        .to_owned()
    }

    fn supported(self) -> bool {
        matches!(
            self,
            Self::Rgba8888
                | Self::Abgr8888
                | Self::Rgb888
                | Self::Bgr888
                | Self::Rgb565
                | Self::I8
                | Self::Ia88
                | Self::A8
                | Self::Rgb888BlueScreen
                | Self::Bgr888BlueScreen
                | Self::Argb8888
                | Self::Bgra8888
                | Self::Dxt1
                | Self::Dxt3
                | Self::Dxt5
                | Self::Dxt1OneBitAlpha
                | Self::Bgrx8888
                | Self::Bgr565
                | Self::Bgrx5551
                | Self::Bgra4444
                | Self::Bgra5551
                | Self::Uv88
                | Self::Uvwq8888
                | Self::Rgba16161616F
                | Self::Rgba16161616
                | Self::Uvlx8888
                | Self::R32F
                | Self::Rgb323232F
                | Self::Rgba32323232F
                | Self::Ati2N
                | Self::Ati1N
        )
    }

    fn storage(self) -> Option<Storage> {
        match self {
            Self::Rgba8888
            | Self::Abgr8888
            | Self::Argb8888
            | Self::Bgra8888
            | Self::Bgrx8888
            | Self::Uvwq8888
            | Self::Uvlx8888
            | Self::R32F => Some(Storage::BytesPerPixel(4)),
            Self::Rgb888 | Self::Bgr888 | Self::Rgb888BlueScreen | Self::Bgr888BlueScreen => {
                Some(Storage::BytesPerPixel(3))
            }
            Self::Rgb565
            | Self::Ia88
            | Self::Bgr565
            | Self::Bgrx5551
            | Self::Bgra4444
            | Self::Bgra5551
            | Self::Uv88 => Some(Storage::BytesPerPixel(2)),
            Self::I8 | Self::P8 | Self::A8 => Some(Storage::BytesPerPixel(1)),
            Self::Dxt1 | Self::Dxt1OneBitAlpha | Self::Ati1N => Some(Storage::Block(8)),
            Self::Dxt3 | Self::Dxt5 | Self::Ati2N => Some(Storage::Block(16)),
            Self::Rgba16161616F | Self::Rgba16161616 => Some(Storage::BytesPerPixel(8)),
            Self::Rgb323232F => Some(Storage::BytesPerPixel(12)),
            Self::Rgba32323232F => Some(Storage::BytesPerPixel(16)),
            Self::NvDst16 | Self::AtiDst16 => Some(Storage::BytesPerPixel(2)),
            Self::NvDst24 | Self::NvIntz | Self::NvRawz | Self::AtiDst24 | Self::NvNull => {
                Some(Storage::BytesPerPixel(4))
            }
            Self::Unknown(_) => None,
        }
    }
}

pub fn vtf_format_universe() -> Vec<VtfFormatMetadata> {
    (0..=38)
        .map(|code| {
            let format = ImageFormat::from_code(code);
            VtfFormatMetadata {
                code,
                name: format.name(),
                supported: format.supported(),
            }
        })
        .collect()
}

#[derive(Clone, Copy, Debug)]
enum Storage {
    BytesPerPixel(usize),
    Block(usize),
}

#[derive(Debug)]
struct ParsedVtf {
    metadata: VtfMetadata,
    format: ImageFormat,
    high_resolution_offset: usize,
    high_resolution_length: Option<usize>,
}

fn read_u16(data: &[u8], offset: usize, field: &str) -> Result<u16, VtfError> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or_else(|| VtfError::invalid(format!("VTF header is truncated at {field}")))?;
    Ok(u16::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_u32(data: &[u8], offset: usize, field: &str) -> Result<u32, VtfError> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| VtfError::invalid(format!("VTF header is truncated at {field}")))?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

fn checked_image_size(
    width: u32,
    height: u32,
    format: ImageFormat,
) -> Result<Option<usize>, VtfError> {
    let Some(storage) = format.storage() else {
        return Ok(None);
    };
    let size = match storage {
        Storage::BytesPerPixel(bytes) => usize::try_from(width)
            .ok()
            .and_then(|width| {
                usize::try_from(height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .and_then(|pixels| pixels.checked_mul(bytes)),
        Storage::Block(bytes) => {
            let block_width = usize::try_from(width.div_ceil(4)).ok();
            let block_height = usize::try_from(height.div_ceil(4)).ok();
            block_width
                .and_then(|width| block_height.and_then(|height| width.checked_mul(height)))
                .and_then(|blocks| blocks.checked_mul(bytes))
        }
    }
    .ok_or_else(|| VtfError::invalid("VTF image byte length overflows"))?;
    Ok(Some(size))
}

fn mip_dimension(value: u32, mip: u8) -> u32 {
    value.checked_shr(u32::from(mip)).unwrap_or(0).max(1)
}

fn high_resolution_length(
    metadata: &VtfMetadata,
    format: ImageFormat,
) -> Result<Option<usize>, VtfError> {
    let mut total = 0_usize;
    for mip in 0..metadata.mip_count {
        let Some(surface) = checked_image_size(
            mip_dimension(metadata.width, mip),
            mip_dimension(metadata.height, mip),
            format,
        )?
        else {
            return Ok(None);
        };
        let depth = usize::try_from(mip_dimension(metadata.depth, mip))
            .map_err(|_| VtfError::invalid("VTF mip depth does not fit this platform"))?;
        let level = surface
            .checked_mul(depth)
            .and_then(|value| value.checked_mul(usize::from(metadata.frames)))
            .and_then(|value| value.checked_mul(usize::from(metadata.faces)))
            .ok_or_else(|| VtfError::invalid("VTF high-resolution image length overflows"))?;
        total = total
            .checked_add(level)
            .ok_or_else(|| VtfError::invalid("VTF high-resolution image length overflows"))?;
        if total > MAX_VTF_BYTES {
            return Err(VtfError::invalid(format!(
                "VTF high-resolution images exceed the {MAX_VTF_BYTES}-byte safety limit"
            )));
        }
    }
    Ok(Some(total))
}

fn validate_range(data: &[u8], offset: usize, length: usize, label: &str) -> Result<(), VtfError> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| VtfError::invalid(format!("VTF {label} range overflows")))?;
    if end > data.len() {
        return Err(VtfError::invalid(format!(
            "VTF {label} is truncated: range {offset}..{end}, file length {}",
            data.len()
        )));
    }
    Ok(())
}

fn resource_tag(tag: [u8; 3]) -> String {
    if tag.iter().all(u8::is_ascii_graphic) {
        String::from_utf8(tag.to_vec()).expect("ASCII VTF resource tag")
    } else {
        format!("0x{:02x}{:02x}{:02x}", tag[0], tag[1], tag[2])
    }
}

fn parse_vtf(data: &[u8]) -> Result<ParsedVtf, VtfError> {
    if data.len() > MAX_VTF_BYTES {
        return Err(VtfError::invalid(format!(
            "VTF exceeds the {MAX_VTF_BYTES}-byte safety limit"
        )));
    }
    if data.get(0..4) != Some(b"VTF\0") {
        return Err(VtfError::invalid("VTF signature is missing"));
    }
    let version_major = read_u32(data, 4, "version major")?;
    let version_minor = read_u32(data, 8, "version minor")?;
    if version_major != 7 || version_minor > 5 {
        return Err(VtfError::unsupported(format!(
            "unsupported VTF version {version_major}.{version_minor}; supported versions are 7.0 through 7.5"
        )));
    }
    let minimum_header_size = if version_minor >= 3 {
        80
    } else if version_minor >= 2 {
        65
    } else {
        64
    };
    let header_size = usize::try_from(read_u32(data, 12, "header size")?)
        .map_err(|_| VtfError::invalid("VTF header size does not fit this platform"))?;
    if header_size < minimum_header_size || header_size > data.len() {
        return Err(VtfError::invalid(format!(
            "VTF header size {header_size} is outside {minimum_header_size}..={} bytes",
            data.len()
        )));
    }
    let width = u32::from(read_u16(data, 16, "width")?);
    let height = u32::from(read_u16(data, 18, "height")?);
    if width == 0 || height == 0 || width > MAX_VTF_DIMENSION || height > MAX_VTF_DIMENSION {
        return Err(VtfError::invalid(format!(
            "VTF dimensions {width}x{height} are outside 1..={MAX_VTF_DIMENSION}"
        )));
    }
    let flags = read_u32(data, 20, "flags")?;
    let frames = read_u16(data, 24, "frame count")?;
    let start_frame = read_u16(data, 26, "start frame")?;
    if frames == 0 {
        return Err(VtfError::invalid("VTF frame count must not be zero"));
    }
    let format_code = read_u32(data, 52, "high-resolution format")?;
    let format = ImageFormat::from_code(format_code);
    let mip_count = *data
        .get(56)
        .ok_or_else(|| VtfError::invalid("VTF header is truncated at mip count"))?;
    let depth = if version_minor >= 2 {
        u32::from(read_u16(data, 63, "depth")?)
    } else {
        1
    };
    if depth == 0 || depth > MAX_VTF_DIMENSION {
        return Err(VtfError::invalid(format!(
            "VTF depth {depth} is outside 1..={MAX_VTF_DIMENSION}"
        )));
    }
    let maximum_mips = 32 - width.max(height).max(depth).leading_zeros();
    if mip_count == 0 || u32::from(mip_count) > maximum_mips {
        return Err(VtfError::invalid(format!(
            "VTF mip count {mip_count} is invalid for {width}x{height}x{depth}"
        )));
    }
    let faces = if flags & TEXTUREFLAGS_ENVMAP != 0 {
        if version_minor < 1 { 6 } else { 7 }
    } else {
        1
    };
    let mut metadata = VtfMetadata {
        version_major,
        version_minor,
        width,
        height,
        depth,
        frames,
        start_frame,
        faces,
        mip_count,
        flags,
        format: VtfFormatMetadata {
            code: format_code,
            name: format.name(),
            supported: format.supported(),
        },
        resources: Vec::new(),
    };

    let low_format_code = read_u32(data, 57, "low-resolution format")?;
    let low_width = u32::from(
        *data
            .get(61)
            .ok_or_else(|| VtfError::invalid("VTF header is truncated at low-resolution width"))?,
    );
    let low_height =
        u32::from(*data.get(62).ok_or_else(|| {
            VtfError::invalid("VTF header is truncated at low-resolution height")
        })?);
    if (low_width == 0) != (low_height == 0) {
        return Err(VtfError::invalid(
            "VTF low-resolution dimensions must both be zero or non-zero",
        ));
    }
    let low_length = if low_width == 0 {
        0
    } else {
        checked_image_size(
            low_width,
            low_height,
            ImageFormat::from_code(low_format_code),
        )?
        .ok_or_else(|| {
            VtfError::unsupported(format!(
                "unsupported VTF low-resolution image format {low_format_code}"
            ))
        })?
    };

    let high_resolution_offset = if version_minor >= 3 {
        let resource_count = usize::try_from(read_u32(data, 68, "resource count")?)
            .map_err(|_| VtfError::invalid("VTF resource count does not fit this platform"))?;
        if resource_count > MAX_RESOURCES {
            return Err(VtfError::invalid(format!(
                "VTF has {resource_count} resources; limit is {MAX_RESOURCES}"
            )));
        }
        let resource_end = 80_usize
            .checked_add(
                resource_count
                    .checked_mul(8)
                    .ok_or_else(|| VtfError::invalid("VTF resource directory length overflows"))?,
            )
            .ok_or_else(|| VtfError::invalid("VTF resource directory length overflows"))?;
        if resource_end > header_size {
            return Err(VtfError::invalid(format!(
                "VTF resource directory ends at {resource_end}, beyond header size {header_size}"
            )));
        }
        let mut high_offset = None;
        let mut low_offset = None;
        let mut previous_type = None;
        for index in 0..resource_count {
            let entry = 80 + index * 8;
            let tag: [u8; 3] = data[entry..entry + 3].try_into().unwrap();
            let resource_flags = data[entry + 3];
            if resource_flags & !RESOURCE_NO_DATA_CHUNK != 0 {
                return Err(VtfError::invalid(format!(
                    "VTF resource {index} has unknown flags {resource_flags:#04x}"
                )));
            }
            let resource_type = u32::from_le_bytes([tag[0], tag[1], tag[2], 0]);
            if previous_type.is_some_and(|previous| previous >= resource_type) {
                return Err(VtfError::invalid(
                    "VTF resource directory is not strictly sorted by resource type",
                ));
            }
            previous_type = Some(resource_type);
            let value = read_u32(data, entry + 4, "resource data offset")?;
            let is_inline = resource_flags & RESOURCE_NO_DATA_CHUNK != 0;
            let offset = (!is_inline)
                .then(|| {
                    usize::try_from(value).map_err(|_| {
                        VtfError::invalid("VTF resource offset does not fit this platform")
                    })
                })
                .transpose()?;
            if let Some(offset) = offset
                && (offset < header_size || offset > data.len())
            {
                return Err(VtfError::invalid(format!(
                    "VTF resource {index} offset {offset} is outside {header_size}..={} bytes",
                    data.len()
                )));
            }
            let mut byte_length = None;
            if let Some(offset) = offset
                && tag != HIGH_RES_IMAGE_RESOURCE
                && tag != LOW_RES_IMAGE_RESOURCE
            {
                let length = usize::try_from(read_u32(data, offset, "resource data length")?)
                    .map_err(|_| {
                        VtfError::invalid("VTF resource length does not fit this platform")
                    })?;
                let body = offset
                    .checked_add(4)
                    .ok_or_else(|| VtfError::invalid("VTF resource body offset overflows"))?;
                validate_range(data, body, length, &format!("resource {index} data"))?;
                byte_length = Some(length);
            }
            if tag == HIGH_RES_IMAGE_RESOURCE {
                if is_inline {
                    return Err(VtfError::invalid(
                        "VTF high-resolution image resource is marked as inline data",
                    ));
                }
                if high_offset
                    .replace(offset.expect("non-inline image resource"))
                    .is_some()
                {
                    return Err(VtfError::invalid(
                        "VTF has duplicate high-resolution image resources",
                    ));
                }
            } else if tag == LOW_RES_IMAGE_RESOURCE {
                if is_inline {
                    return Err(VtfError::invalid(
                        "VTF low-resolution image resource is marked as inline data",
                    ));
                }
                if low_offset
                    .replace(offset.expect("non-inline image resource"))
                    .is_some()
                {
                    return Err(VtfError::invalid(
                        "VTF has duplicate low-resolution image resources",
                    ));
                }
            }
            metadata.resources.push(VtfResourceMetadata {
                tag: resource_tag(tag),
                flags: resource_flags,
                is_inline,
                inline_data: is_inline.then_some(value),
                offset,
                byte_length,
            });
        }
        if low_length > 0 {
            let offset = low_offset.ok_or_else(|| {
                VtfError::invalid("VTF resource directory is missing the low-resolution image")
            })?;
            validate_range(data, offset, low_length, "low-resolution image")?;
        }
        high_offset.ok_or_else(|| {
            VtfError::invalid("VTF resource directory is missing the high-resolution image")
        })?
    } else {
        validate_range(data, header_size, low_length, "low-resolution image")?;
        header_size
            .checked_add(low_length)
            .ok_or_else(|| VtfError::invalid("VTF high-resolution image offset overflows"))?
    };
    let high_resolution_length = high_resolution_length(&metadata, format)?;
    if let Some(length) = high_resolution_length {
        validate_range(
            data,
            high_resolution_offset,
            length,
            "high-resolution image data",
        )?;
    }

    Ok(ParsedVtf {
        metadata,
        format,
        high_resolution_offset,
        high_resolution_length,
    })
}

pub fn inspect_vtf(data: &[u8]) -> Result<VtfMetadata, VtfError> {
    parse_vtf(data).map(|parsed| parsed.metadata)
}

fn selected_image_range(
    parsed: &ParsedVtf,
    selection: VtfImageSelection,
) -> Result<(usize, usize, u32, u32), VtfError> {
    let metadata = &parsed.metadata;
    if selection.mip >= metadata.mip_count {
        return Err(VtfError::invalid(format!(
            "VTF mip {} is out of range for {} mips",
            selection.mip, metadata.mip_count
        )));
    }
    if selection.frame >= metadata.frames {
        return Err(VtfError::invalid(format!(
            "VTF frame {} is out of range for {} frames",
            selection.frame, metadata.frames
        )));
    }
    if selection.face >= metadata.faces {
        return Err(VtfError::invalid(format!(
            "VTF face {} is out of range for {} faces",
            selection.face, metadata.faces
        )));
    }
    parsed.high_resolution_length.ok_or_else(|| {
        VtfError::unsupported(format!(
            "unsupported VTF image format {} ({})",
            metadata.format.code, metadata.format.name
        ))
    })?;
    let mut offset = parsed.high_resolution_offset;
    for mip in (0..metadata.mip_count).rev() {
        let width = mip_dimension(metadata.width, mip);
        let height = mip_dimension(metadata.height, mip);
        let depth = mip_dimension(metadata.depth, mip);
        let surface = checked_image_size(width, height, parsed.format)?
            .expect("known high-resolution storage");
        let level_length = surface
            .checked_mul(
                usize::try_from(depth)
                    .map_err(|_| VtfError::invalid("VTF mip depth does not fit this platform"))?,
            )
            .and_then(|value| value.checked_mul(usize::from(metadata.frames)))
            .and_then(|value| value.checked_mul(usize::from(metadata.faces)))
            .ok_or_else(|| VtfError::invalid("VTF mip image range overflows"))?;
        if mip == selection.mip {
            if u32::from(selection.slice) >= depth {
                return Err(VtfError::invalid(format!(
                    "VTF slice {} is out of range for mip {} depth {depth}",
                    selection.slice, selection.mip
                )));
            }
            let image_index = usize::from(selection.frame)
                .checked_mul(usize::from(metadata.faces))
                .and_then(|value| value.checked_add(usize::from(selection.face)))
                .and_then(|value| value.checked_mul(depth as usize))
                .and_then(|value| value.checked_add(usize::from(selection.slice)))
                .ok_or_else(|| VtfError::invalid("VTF selected image index overflows"))?;
            let selected_offset = offset
                .checked_add(
                    image_index
                        .checked_mul(surface)
                        .ok_or_else(|| VtfError::invalid("VTF selected image offset overflows"))?,
                )
                .ok_or_else(|| VtfError::invalid("VTF selected image offset overflows"))?;
            return Ok((selected_offset, surface, width, height));
        }
        offset = offset
            .checked_add(level_length)
            .ok_or_else(|| VtfError::invalid("VTF mip image range overflows"))?;
    }
    unreachable!("validated VTF mip selection was not found")
}

fn rgba_length(width: u32, height: u32) -> Result<usize, VtfError> {
    let length = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| VtfError::invalid("decoded VTF RGBA length overflows"))?;
    if length > MAX_DECODED_BYTES {
        return Err(VtfError::invalid(format!(
            "decoded VTF RGBA data exceeds the {MAX_DECODED_BYTES}-byte safety limit"
        )));
    }
    Ok(length)
}

fn float_to_unorm8(value: f32) -> Result<u8, VtfError> {
    if !value.is_finite() {
        return Err(VtfError::invalid(
            "VTF floating-point image contains a non-finite value",
        ));
    }
    Ok((value.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn decode_uncompressed(
    format: ImageFormat,
    encoded: &[u8],
    pixels: &mut [u8],
) -> Result<(), VtfError> {
    match format {
        ImageFormat::Rgba8888 => pixels.copy_from_slice(encoded),
        ImageFormat::Abgr8888 => {
            for (source, output) in encoded.chunks_exact(4).zip(pixels.chunks_exact_mut(4)) {
                output.copy_from_slice(&[source[3], source[2], source[1], source[0]]);
            }
        }
        ImageFormat::Rgb888 => {
            for (source, output) in encoded.chunks_exact(3).zip(pixels.chunks_exact_mut(4)) {
                output.copy_from_slice(&[source[0], source[1], source[2], 255]);
            }
        }
        ImageFormat::Bgr888 => {
            for (source, output) in encoded.chunks_exact(3).zip(pixels.chunks_exact_mut(4)) {
                output.copy_from_slice(&[source[2], source[1], source[0], 255]);
            }
        }
        ImageFormat::Rgb565 => {
            for (source, output) in encoded.chunks_exact(2).zip(pixels.chunks_exact_mut(4)) {
                let value = u16::from_le_bytes([source[0], source[1]]);
                let red = value & 0x1f;
                let green = (value >> 5) & 0x3f;
                let blue = (value >> 11) & 0x1f;
                output.copy_from_slice(&[
                    ((red << 3) | (red >> 2)) as u8,
                    ((green << 2) | (green >> 4)) as u8,
                    ((blue << 3) | (blue >> 2)) as u8,
                    255,
                ]);
            }
        }
        ImageFormat::Rgb888BlueScreen | ImageFormat::Bgr888BlueScreen => {
            for (source, output) in encoded.chunks_exact(3).zip(pixels.chunks_exact_mut(4)) {
                let rgb = if format == ImageFormat::Rgb888BlueScreen {
                    [source[0], source[1], source[2]]
                } else {
                    [source[2], source[1], source[0]]
                };
                if rgb == [0, 0, 255] {
                    output.fill(0);
                } else {
                    output.copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
                }
            }
        }
        ImageFormat::Argb8888 => {
            for (source, output) in encoded.chunks_exact(4).zip(pixels.chunks_exact_mut(4)) {
                output.copy_from_slice(&[source[1], source[2], source[3], source[0]]);
            }
        }
        ImageFormat::Bgra8888 => {
            for (source, output) in encoded.chunks_exact(4).zip(pixels.chunks_exact_mut(4)) {
                output.copy_from_slice(&[source[2], source[1], source[0], source[3]]);
            }
        }
        ImageFormat::I8 => {
            for (&intensity, output) in encoded.iter().zip(pixels.chunks_exact_mut(4)) {
                output.copy_from_slice(&[intensity, intensity, intensity, 255]);
            }
        }
        ImageFormat::Ia88 => {
            for (source, output) in encoded.chunks_exact(2).zip(pixels.chunks_exact_mut(4)) {
                output.copy_from_slice(&[source[0], source[0], source[0], source[1]]);
            }
        }
        ImageFormat::A8 => {
            for (&alpha, output) in encoded.iter().zip(pixels.chunks_exact_mut(4)) {
                output.fill(alpha);
            }
        }
        ImageFormat::Bgrx8888 => {
            for (source, output) in encoded.chunks_exact(4).zip(pixels.chunks_exact_mut(4)) {
                output.copy_from_slice(&[source[2], source[1], source[0], 255]);
            }
        }
        ImageFormat::Bgr565
        | ImageFormat::Bgrx5551
        | ImageFormat::Bgra5551
        | ImageFormat::Bgra4444 => {
            for (source, output) in encoded.chunks_exact(2).zip(pixels.chunks_exact_mut(4)) {
                let value = u16::from_le_bytes([source[0], source[1]]);
                let (red, green, blue, alpha) = match format {
                    ImageFormat::Bgr565 => {
                        ((value >> 11) & 0x1f, (value >> 5) & 0x3f, value & 0x1f, 255)
                    }
                    ImageFormat::Bgrx5551 => {
                        ((value >> 10) & 0x1f, (value >> 5) & 0x1f, value & 0x1f, 255)
                    }
                    ImageFormat::Bgra5551 => (
                        (value >> 10) & 0x1f,
                        (value >> 5) & 0x1f,
                        value & 0x1f,
                        if value & 0x8000 != 0 { 255 } else { 0 },
                    ),
                    ImageFormat::Bgra4444 => {
                        output.copy_from_slice(&[
                            ((value >> 8) & 0x0f) as u8 * 16,
                            ((value >> 4) & 0x0f) as u8 * 16,
                            (value & 0x0f) as u8 * 16,
                            ((value >> 12) & 0x0f) as u8 * 16,
                        ]);
                        continue;
                    }
                    _ => unreachable!(),
                };
                let expand_five = |channel: u16| ((channel << 3) | (channel >> 2)) as u8;
                output.copy_from_slice(&[
                    expand_five(red),
                    if format == ImageFormat::Bgr565 {
                        ((green << 2) | (green >> 4)) as u8
                    } else {
                        expand_five(green)
                    },
                    expand_five(blue),
                    alpha as u8,
                ]);
            }
        }
        ImageFormat::Uv88 => {
            for (source, output) in encoded.chunks_exact(2).zip(pixels.chunks_exact_mut(4)) {
                output.copy_from_slice(&[source[0], source[1], 0, 0]);
            }
        }
        ImageFormat::Uvwq8888 | ImageFormat::Uvlx8888 => {
            pixels.copy_from_slice(encoded);
        }
        ImageFormat::Rgba16161616 => {
            for (source, output) in encoded.chunks_exact(8).zip(pixels.chunks_exact_mut(4)) {
                let channel = |offset| u16::from_le_bytes([source[offset], source[offset + 1]]);
                output.copy_from_slice(&[
                    (channel(0) >> 4).min(255) as u8,
                    (channel(2) >> 4).min(255) as u8,
                    (channel(4) >> 4).min(255) as u8,
                    (channel(6) >> 8).min(255) as u8,
                ]);
            }
        }
        ImageFormat::Rgba16161616F => {
            for (source, output) in encoded.chunks_exact(8).zip(pixels.chunks_exact_mut(4)) {
                for channel in 0..4 {
                    output[channel] = float_to_unorm8(
                        half::f16::from_bits(u16::from_le_bytes([
                            source[channel * 2],
                            source[channel * 2 + 1],
                        ]))
                        .to_f32(),
                    )?;
                }
            }
        }
        ImageFormat::R32F | ImageFormat::Rgb323232F | ImageFormat::Rgba32323232F => {
            let channels = match format {
                ImageFormat::R32F => 1,
                ImageFormat::Rgb323232F => 3,
                ImageFormat::Rgba32323232F => 4,
                _ => unreachable!(),
            };
            for (source, output) in encoded
                .chunks_exact(channels * 4)
                .zip(pixels.chunks_exact_mut(4))
            {
                output.fill(0);
                output[3] = 255;
                for channel in 0..channels {
                    output[channel] = float_to_unorm8(f32::from_le_bytes(
                        source[channel * 4..channel * 4 + 4].try_into().unwrap(),
                    ))?;
                }
            }
        }
        _ => unreachable!("unsupported uncompressed VTF format"),
    }
    Ok(())
}

fn decode_bc_alpha_block(block: &[u8]) -> [u8; 16] {
    let palette = alpha_palette_dxt5(block);
    let bits = u64::from_le_bytes([
        block[2], block[3], block[4], block[5], block[6], block[7], 0, 0,
    ]);
    std::array::from_fn(|index| palette[((bits >> (index * 3)) & 7) as usize])
}

fn decode_ati(format: ImageFormat, encoded: &[u8], width: u32, height: u32, pixels: &mut [u8]) {
    let block_size = if format == ImageFormat::Ati2N { 16 } else { 8 };
    let blocks_wide = width.div_ceil(4) as usize;
    for (block_index, block) in encoded.chunks_exact(block_size).enumerate() {
        let block_x = block_index % blocks_wide;
        let block_y = block_index / blocks_wide;
        let red = decode_bc_alpha_block(&block[0..8]);
        let green = (format == ImageFormat::Ati2N).then(|| decode_bc_alpha_block(&block[8..16]));
        for texel in 0..16 {
            let x = block_x * 4 + texel % 4;
            let y = block_y * 4 + texel / 4;
            if x >= width as usize || y >= height as usize {
                continue;
            }
            let output = (y * width as usize + x) * 4;
            pixels[output..output + 4].copy_from_slice(&[
                red[texel],
                green.as_ref().map_or(0, |values| values[texel]),
                0,
                0,
            ]);
        }
    }
}

fn expand_565(value: u16) -> [u8; 4] {
    let red = ((value >> 11) & 0x1f) as u8;
    let green = ((value >> 5) & 0x3f) as u8;
    let blue = (value & 0x1f) as u8;
    [
        (red << 3) | (red >> 2),
        (green << 2) | (green >> 4),
        (blue << 3) | (blue >> 2),
        255,
    ]
}

fn color_palette(block: &[u8], one_bit_alpha: bool) -> [[u8; 4]; 4] {
    let color0 = u16::from_le_bytes([block[0], block[1]]);
    let color1 = u16::from_le_bytes([block[2], block[3]]);
    let first = expand_565(color0);
    let second = expand_565(color1);
    let mut colors = [first, second, [0; 4], [0; 4]];
    if color0 > color1 || !one_bit_alpha {
        for channel in 0..3 {
            colors[2][channel] =
                ((2 * u16::from(first[channel]) + u16::from(second[channel])) / 3) as u8;
            colors[3][channel] =
                ((u16::from(first[channel]) + 2 * u16::from(second[channel])) / 3) as u8;
        }
        colors[2][3] = 255;
        colors[3][3] = 255;
    } else {
        for channel in 0..3 {
            colors[2][channel] =
                ((u16::from(first[channel]) + u16::from(second[channel])) / 2) as u8;
        }
        colors[2][3] = 255;
    }
    colors
}

fn alpha_palette_dxt5(block: &[u8]) -> [u8; 8] {
    let first = block[0];
    let second = block[1];
    let mut alpha = [first, second, 0, 0, 0, 0, 0, 0];
    if first > second {
        for (index, output) in alpha.iter_mut().enumerate().skip(2) {
            *output = (((8 - index) as u16 * u16::from(first)
                + (index - 1) as u16 * u16::from(second))
                / 7) as u8;
        }
    } else {
        for (index, output) in alpha.iter_mut().enumerate().take(6).skip(2) {
            *output = (((6 - index) as u16 * u16::from(first)
                + (index - 1) as u16 * u16::from(second))
                / 5) as u8;
        }
        alpha[6] = 0;
        alpha[7] = 255;
    }
    alpha
}

fn decode_dxt(format: ImageFormat, encoded: &[u8], width: u32, height: u32, pixels: &mut [u8]) {
    let block_size = if matches!(format, ImageFormat::Dxt1 | ImageFormat::Dxt1OneBitAlpha) {
        8
    } else {
        16
    };
    let blocks_wide = width.div_ceil(4) as usize;
    for (block_index, block) in encoded.chunks_exact(block_size).enumerate() {
        let block_x = block_index % blocks_wide;
        let block_y = block_index / blocks_wide;
        let (color_block, color_bits, alpha_bits, alpha_palette) = match format {
            ImageFormat::Dxt1 | ImageFormat::Dxt1OneBitAlpha => {
                let bits = u32::from_le_bytes(block[4..8].try_into().unwrap());
                (&block[0..8], bits, 0, None)
            }
            ImageFormat::Dxt3 => {
                let bits = u32::from_le_bytes(block[12..16].try_into().unwrap());
                let alpha = u64::from_le_bytes(block[0..8].try_into().unwrap());
                (&block[8..16], bits, alpha, None)
            }
            ImageFormat::Dxt5 => {
                let bits = u32::from_le_bytes(block[12..16].try_into().unwrap());
                let alpha = u64::from_le_bytes([
                    block[2], block[3], block[4], block[5], block[6], block[7], 0, 0,
                ]);
                (&block[8..16], bits, alpha, Some(alpha_palette_dxt5(block)))
            }
            _ => unreachable!("unsupported DXT format"),
        };
        let colors = color_palette(
            color_block,
            matches!(format, ImageFormat::Dxt1 | ImageFormat::Dxt1OneBitAlpha),
        );
        for texel in 0..16 {
            let x = block_x * 4 + texel % 4;
            let y = block_y * 4 + texel / 4;
            if x >= width as usize || y >= height as usize {
                continue;
            }
            let color_index = ((color_bits >> (texel * 2)) & 3) as usize;
            let mut color = colors[color_index];
            match format {
                ImageFormat::Dxt3 => color[3] = (((alpha_bits >> (texel * 4)) & 0xf) * 17) as u8,
                ImageFormat::Dxt5 => {
                    let alpha_index = ((alpha_bits >> (texel * 3)) & 7) as usize;
                    color[3] = alpha_palette.unwrap()[alpha_index];
                }
                _ => {}
            }
            let output = (y * width as usize + x) * 4;
            pixels[output..output + 4].copy_from_slice(&color);
        }
    }
}

pub fn decode_vtf(data: &[u8], selection: VtfImageSelection) -> Result<DecodedVtf, VtfError> {
    let parsed = parse_vtf(data)?;
    if !parsed.format.supported() {
        return Err(VtfError::unsupported(format!(
            "unsupported VTF image format {} ({})",
            parsed.metadata.format.code, parsed.metadata.format.name
        )));
    }
    let (offset, length, width, height) = selected_image_range(&parsed, selection)?;
    validate_range(data, offset, length, "selected high-resolution image")?;
    let encoded = &data[offset..offset + length];
    let mut pixels = vec![0; rgba_length(width, height)?];
    match parsed.format {
        ImageFormat::Dxt1
        | ImageFormat::Dxt1OneBitAlpha
        | ImageFormat::Dxt3
        | ImageFormat::Dxt5 => decode_dxt(parsed.format, encoded, width, height, &mut pixels),
        ImageFormat::Ati2N | ImageFormat::Ati1N => {
            decode_ati(parsed.format, encoded, width, height, &mut pixels)
        }
        _ => decode_uncompressed(parsed.format, encoded, &mut pixels)?,
    }
    Ok(DecodedVtf {
        metadata: parsed.metadata,
        selection,
        width,
        height,
        pixels,
    })
}
