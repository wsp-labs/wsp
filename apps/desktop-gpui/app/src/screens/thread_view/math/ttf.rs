// SPDX-License-Identifier: AGPL-3.0-only
//! Just enough of a TrueType reader to take a KaTeX_Size glyph's outline.

/// Just enough of a TrueType font to read a glyph's outline: `cmap` format 4, `loca` and `glyf`.
pub(super) struct Ttf<'a> {
    data: &'a [u8],
    cmap: usize,
    loca: usize,
    glyf: usize,
    long: bool,
    units: f32,
}

impl<'a> Ttf<'a> {
    fn u16(&self, at: usize) -> Option<u16> {
        Some(u16::from_be_bytes(self.data.get(at..at + 2)?.try_into().ok()?))
    }

    fn i16(&self, at: usize) -> Option<i16> {
        self.u16(at).map(|v| v as i16)
    }

    fn u32(&self, at: usize) -> Option<usize> {
        Some(u32::from_be_bytes(self.data.get(at..at + 4)?.try_into().ok()?) as usize)
    }

    pub(super) fn new(data: &'a [u8]) -> Option<Self> {
        let mut font = Ttf { data, cmap: 0, loca: 0, glyf: 0, long: false, units: 1000. };
        let mut head = 0;
        for table in 0..font.u16(4)? as usize {
            let record = 12 + 16 * table;
            let offset = font.u32(record + 8)?;
            match data.get(record..record + 4)? {
                b"cmap" => font.cmap = offset,
                b"loca" => font.loca = offset,
                b"glyf" => font.glyf = offset,
                b"head" => head = offset,
                _ => {}
            }
        }
        font.units = f32::from(font.u16(head + 18)?);
        font.long = font.i16(head + 50)? == 1;
        Some(font)
    }

    fn glyph(&self, ch: char) -> Option<u16> {
        let c = u16::try_from(u32::from(ch)).ok()?;
        for record in 0..self.u16(self.cmap + 2)? as usize {
            let table = self.cmap + self.u32(self.cmap + 4 + 8 * record + 4)?;
            if self.u16(table)? != 4 {
                continue;
            }
            let segments = self.u16(table + 6)? as usize;
            let ends = table + 14;
            let (starts, deltas, ranges) = (ends + segments + 2, ends + 2 * segments + 2, ends + 3 * segments + 2);
            for seg in (0..segments).step_by(2) {
                if c > self.u16(ends + seg)? {
                    continue;
                }
                let start = self.u16(starts + seg)?;
                if c < start {
                    return None;
                }
                let delta = self.u16(deltas + seg)?;
                let range = self.u16(ranges + seg)? as usize;
                if range == 0 {
                    return Some(c.wrapping_add(delta));
                }
                let glyph = self.u16(ranges + seg + range + 2 * usize::from(c - start))?;
                return (glyph != 0).then(|| glyph.wrapping_add(delta));
            }
        }
        None
    }

    /// The glyph's contours: points in em, `true` when on the curve.
    pub(super) fn outline(&self, ch: char) -> Option<Vec<Vec<(f32, f32, bool)>>> {
        let mut contours = Vec::new();
        self.contours(self.glyph(ch)?, (0., 0., 1.), &mut contours, 0)?;
        Some(contours)
    }

    fn contours(&self, glyph: u16, (dx, dy, scale): (f32, f32, f32), out: &mut Vec<Vec<(f32, f32, bool)>>, depth: u8) -> Option<()> {
        let glyph = usize::from(glyph);
        let (start, end) = if self.long {
            (self.u32(self.loca + 4 * glyph)?, self.u32(self.loca + 4 * glyph + 4)?)
        } else {
            (2 * self.u16(self.loca + 2 * glyph)? as usize, 2 * self.u16(self.loca + 2 * glyph + 2)? as usize)
        };
        if start == end {
            return Some(());
        }
        let at = self.glyf + start;
        let count = self.i16(at)?;
        let point = |x: i32, y: i32, on: bool| (dx + x as f32 * scale / self.units, dy + y as f32 * scale / self.units, on);
        if count < 0 {
            // A composite: its parts, each offset and maybe scaled.
            let mut p = at + 10;
            loop {
                if depth > 4 {
                    return None;
                }
                let (flags, part) = (self.u16(p)?, self.u16(p + 2)?);
                p += 4;
                let (x, y) = if flags & 1 != 0 {
                    p += 4;
                    (f32::from(self.i16(p - 4)?), f32::from(self.i16(p - 2)?))
                } else {
                    p += 2;
                    (f32::from(*self.data.get(p - 2)? as i8), f32::from(*self.data.get(p - 1)? as i8))
                };
                let mut part_scale = 1.;
                if flags & 8 != 0 {
                    part_scale = f32::from(self.i16(p)?) / 16384.;
                    p += 2;
                } else if flags & 0x40 != 0 {
                    part_scale = f32::from(self.i16(p)?) / 16384.;
                    p += 4;
                } else if flags & 0x80 != 0 {
                    part_scale = f32::from(self.i16(p)?) / 16384.;
                    p += 8;
                }
                let offset = (dx + x * scale / self.units, dy + y * scale / self.units, scale * part_scale);
                self.contours(part, offset, out, depth + 1)?;
                if flags & 0x20 == 0 {
                    return Some(());
                }
            }
        }
        let count = count as usize;
        let mut ends = Vec::with_capacity(count);
        for contour in 0..count {
            ends.push(self.u16(at + 10 + 2 * contour)? as usize);
        }
        let points = ends.last().map_or(0, |last| last + 1);
        let mut p = at + 12 + 2 * count + self.u16(at + 10 + 2 * count)? as usize;
        let mut flags = Vec::with_capacity(points);
        while flags.len() < points {
            let flag = *self.data.get(p)?;
            p += 1;
            flags.push(flag);
            if flag & 8 != 0 {
                let repeat = *self.data.get(p)?;
                p += 1;
                flags.extend(std::iter::repeat_n(flag, usize::from(repeat)));
            }
        }
        let mut read = |short: u8, same: u8| -> Option<Vec<i32>> {
            let mut value = 0i32;
            let mut values = Vec::with_capacity(points);
            for &flag in &flags[..points] {
                if flag & short != 0 {
                    let delta = i32::from(*self.data.get(p)?);
                    p += 1;
                    value += if flag & same != 0 { delta } else { -delta };
                } else if flag & same == 0 {
                    value += i32::from(self.i16(p)?);
                    p += 2;
                }
                values.push(value);
            }
            Some(values)
        };
        let xs = read(2, 16)?;
        let ys = read(4, 32)?;
        let mut first = 0;
        for end in ends {
            out.push((first..=end).map(|i| point(xs[i], ys[i], flags[i] & 1 != 0)).collect());
            first = end + 1;
        }
        Some(())
    }
}
