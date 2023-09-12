use crate::{Float, StreamReader, StreamWriter};
use anyhow::Result;

pub struct FloatToU32 {
    scale: Float,
}
impl FloatToU32 {
    pub fn new(scale: Float) -> Self {
        Self { scale }
    }
    pub fn work(
        &mut self,
        r: &mut dyn StreamReader<Float>,
        w: &mut dyn StreamWriter<u32>,
    ) -> Result<()> {
        let v: Vec<u32> = r
            .buffer()
            .iter()
            .map(|e| (*e * self.scale) as u32)
            .collect();
        w.write(&v)
    }
}

/*
struct Convert<From, To> {
    scale_from: From,
    scale_to: To,
}
impl std::convert::Into<u32> for Float {
    fn into(t: Float) -> u32 {
        t as u32
    }
}
impl<From, To> Convert<From, To>
where From: std::ops::Mul<Output=From> + std::convert::TryInto<To>,
      To: std::ops::Mul<Output=To>
{
    fn new(scale_from: From, scale_to: To) -> Self {
        Self{
            scale_from,
            scale_to,
        }
    }
    pub fn work(&mut self, r: &mut Stream<From>, w: &mut Stream<To>) -> Result<()>
    where <From as TryInto<To>>::Error: std::fmt::Debug
    {
        let v = r.data.iter().map(|e| {
            //From::into(*e * self.scale_from) * self.scale_to
            (*e * self.scale_from).try_into().unwrap() * self.scale_to
        });
        Ok(())
    }
}
*/
