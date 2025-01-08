use std::str::FromStr;

use anyhow::Context;
use fraction::GenericFraction;
use fraction::ToPrimitive;

use crate::parse::GetAndParse;
use crate::parser::DataBlock;

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Axis {
    X,
    Y,
    Z,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct SymmetryEquivTransformColumn {
    axis: Axis,
    sign: i8,
    translation: GenericFraction<u64>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct SymmetryEquivTransform(pub [SymmetryEquivTransformColumn; 3]);

impl std::ops::Deref for SymmetryEquivTransform {
    type Target = [SymmetryEquivTransformColumn; 3];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl SymmetryEquivTransform {
    pub fn transform_point<T: num_traits::Float>(&self, point: [T; 3]) -> anyhow::Result<[T; 3]> {
        let mut new_point = [T::zero(); 3];

        for (index, column) in self.0.iter().enumerate() {
            let value = match column.axis {
                Axis::X => point[0],
                Axis::Y => point[1],
                Axis::Z => point[2],
            };

            let translation: f64 = column
                .translation
                .to_f64()
                .context("Failed to convert translation to f64")?;

            let translation = T::from(translation).context("Failed to convert translation to T")?;

            let sign = T::from(column.sign).context("Failed to convert sign to T")?;

            new_point[index] = value * sign + translation;
        }

        Ok(new_point)
    }
}

#[cfg(test)]
mod test_symmetry_equiv_transform {
    #[test]
    fn test_transform_point() {
        use fraction::Ratio;

        use crate::symmetry::{Axis, SymmetryEquivTransform, SymmetryEquivTransformColumn};

        let transform = SymmetryEquivTransform([
            SymmetryEquivTransformColumn {
                axis: Axis::Z,
                sign: 1,
                translation: fraction::GenericFraction::Rational(
                    fraction::Sign::Plus,
                    Ratio::new(1, 4),
                ),
            },
            SymmetryEquivTransformColumn {
                axis: Axis::X,
                sign: 1,
                translation: fraction::GenericFraction::Rational(
                    fraction::Sign::Plus,
                    Ratio::new(1, 4),
                ),
            },
            SymmetryEquivTransformColumn {
                axis: Axis::Y,
                sign: 1,
                translation: fraction::GenericFraction::Rational(
                    fraction::Sign::Plus,
                    Ratio::new(0, 1),
                ),
            },
        ]);

        let point = [0.0, 1.0, 1.0];

        let new_point = transform.transform_point(point).unwrap();

        assert_eq!(new_point, [1.25, 0.25, 1.0]);
    }
}

// https://www.iucr.org/__data/iucr/cifdic_html/1/cif_core.dic/Ispace_group_symop_operation_xyz.html
// https://www.iucr.org/__data/iucr/cifdic_html/1/cif_core.dic/Isymmetry_equiv_pos_as_xyz.html
// TODO: idk they seem identical? what's the difference? the name?
#[derive(Debug, Default, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct SymmetryEquivPosAsXYZ(pub Vec<SymmetryEquivTransform>);

impl SymmetryEquivPosAsXYZ {
    pub fn generate_equiv_positions<T: num_traits::Float>(
        &self,
        point: [T; 3],
    ) -> anyhow::Result<Vec<[T; 3]>> {
        let mut points = Vec::new();

        for transform in &self.0 {
            points.push(transform.transform_point(point)?);
        }

        points.sort_by(|a, b| a.partial_cmp(b).unwrap());
        points.dedup();

        Ok(points)
    }
}

impl TryFrom<&DataBlock> for SymmetryEquivPosAsXYZ {
    type Error = anyhow::Error;

    fn try_from(map: &DataBlock) -> anyhow::Result<Self> {
        let raw = map
            .get_and_parse_all::<String>("_space_group_symop_operation_xyz")
            .unwrap_or(map.get_and_parse_all::<String>("_symmetry_equiv_pos_as_xyz")?);

        let mut symmetry_equiv_pos_as_xyz = Vec::new();

        for pos in raw {
            let mut split = pos.split(",");

            let first: SymmetryEquivTransformColumn = split.next().unwrap().try_into()?;
            let second: SymmetryEquivTransformColumn = split.next().unwrap().try_into()?;
            let third: SymmetryEquivTransformColumn = split.next().unwrap().try_into()?;

            if split.next().is_some() {
                return Err(anyhow::anyhow!("Got more than 3 columns"));
            }

            symmetry_equiv_pos_as_xyz.push(SymmetryEquivTransform([first, second, third]));
        }

        Ok(Self(symmetry_equiv_pos_as_xyz))
    }
}

impl TryFrom<&str> for SymmetryEquivTransformColumn {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let parts = TranslationSplit::new(value.trim());

        let mut axis: Option<Axis> = None;
        let mut sign: Option<i8> = None;
        let mut add = None;

        parts.into_iter().for_each(|operation| {
            match operation.to_lowercase().as_str() {
                // TODO: could probably be done nicer
                "+x" => {
                    axis = Some(Axis::X);
                    sign = Some(1);
                }
                "x" => {
                    axis = Some(Axis::X);
                    sign = Some(1);
                }
                "-x" => {
                    axis = Some(Axis::X);
                    sign = Some(-1);
                }
                "+y" => {
                    axis = Some(Axis::Y);
                    sign = Some(1);
                }
                "y" => {
                    axis = Some(Axis::Y);
                    sign = Some(1);
                }
                "-y" => {
                    axis = Some(Axis::Y);
                    sign = Some(-1);
                }
                "+z" => {
                    axis = Some(Axis::Z);
                    sign = Some(1);
                }
                "z" => {
                    axis = Some(Axis::Z);
                    sign = Some(1);
                }
                "-z" => {
                    axis = Some(Axis::Z);
                    sign = Some(-1);
                }
                _ => {
                    add = Some(fraction::Fraction::from_str(operation).unwrap());
                }
            }
        });

        let axis = axis.context("Got no axis")?;
        let sign = sign.context("Got no sign")?;
        let translation = add.unwrap_or_default();

        Ok(SymmetryEquivTransformColumn {
            axis,
            sign,
            translation,
        })
    }
}

#[cfg(test)]
mod test_symmetry_equiv_pos_as_xyz {
    use fraction::Ratio;

    use crate::{
        symmetry::{Axis, SymmetryEquivTransformColumn},
        Parser,
    };

    use super::SymmetryEquivPosAsXYZ;

    #[test]
    fn test_parse() {
        let bytes = std::fs::read(r"assets\diamond.cif").unwrap();

        let data = Parser::new(&bytes).parse();

        let sym: SymmetryEquivPosAsXYZ = data.first_key_value().unwrap().1.try_into().unwrap();

        let expected_first = SymmetryEquivTransformColumn {
            axis: Axis::Z,
            sign: 1,
            translation: fraction::GenericFraction::Rational(
                fraction::Sign::Plus,
                Ratio::new(1, 4),
            ),
        };

        let expected_last = SymmetryEquivTransformColumn {
            axis: Axis::Z,
            sign: 1,
            translation: fraction::GenericFraction::Rational(
                fraction::Sign::Plus,
                Ratio::new(0, 1),
            ),
        };

        assert_eq!(sym.0.first().unwrap()[0], expected_first);
        assert_eq!(sym.0.last().unwrap()[2], expected_last);
    }

    #[test]
    fn test_generate_equiv_positions() {
        let bytes = std::fs::read(r"assets\BaTiO3.cif").unwrap();

        let data = Parser::new(&bytes).parse();

        let phase = data.first_key_value().unwrap().1.try_into_phase().unwrap();

        let sym: SymmetryEquivPosAsXYZ = data.first_key_value().unwrap().1.try_into().unwrap();

        let point = [phase.atoms[1].x, phase.atoms[1].y, phase.atoms[1].z];

        let points = sym.generate_equiv_positions(point).unwrap();

        let expected = [
            [-0.5, -0.5, -0.5],
            [-0.5, -0.5, 0.5],
            [-0.5, 0.5, -0.5],
            [-0.5, 0.5, 0.5],
            [0.5, -0.5, -0.5],
            [0.5, -0.5, 0.5],
            [0.5, 0.5, -0.5],
            [0.5, 0.5, 0.5],
        ];

        for point in points {
            assert!(expected.contains(&point));
        }
    }
}

struct TranslationSplit<'a> {
    char_indices: std::str::CharIndices<'a>,
    chunk_start: usize,
    s: &'a str,
}

impl<'a> TranslationSplit<'a> {
    pub fn new(s: &'a str) -> Self {
        let mut char_indices = s.char_indices();

        char_indices.next();
        Self {
            char_indices,
            chunk_start: 0,
            s,
        }
    }
}

impl<'a> Iterator for TranslationSplit<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        // The input is exhausted
        if self.chunk_start == self.s.len() {
            return None;
        }
        // Find the next uppercase letter position OR the end of the string
        let chunk_end = if let Some((chunk_end, _)) = self
            .char_indices
            .by_ref()
            .skip_while(|(_, c)| !(c == &'+' || c == &'-'))
            .next()
        {
            chunk_end
        } else {
            self.s.len()
        };
        let chunk = &self.s[self.chunk_start..chunk_end];
        self.chunk_start = chunk_end;
        Some(chunk)
    }
}
