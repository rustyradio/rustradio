/*! RustRadio Block implementation

Blocks are the main buildingblocks of rustradio. They each do one
thing, and you connect them together with streams to process the data.

*/

use anyhow::Result;

use crate::Error;

/** Return type for all blocks.

This will let the scheduler know if more data could come out of this block, or if
it should just never bother calling it again.

TODO: Add state for "don't call me unless there's more input".
*/
pub enum BlockRet {
    /// The normal return. More data may be produced only if more data
    /// comes in.
    Ok,

    /// Produced nothing.
    Noop,

    // More data may be produced even if no more data comes in.
    // Currently not used.
    // Background,
    /// Block indicates that it will never produce more input.
    ///
    /// Examples:
    /// * reading from file, without repeating, and file reached EOF.
    /// * Head block reached its max.
    EOF,
}

/**
Block trait, that must be implemented for all blocks.

Simpler blocks can use macros to avoid needing to implement `work()`.
*/
pub trait Block {
    /** Name of block

    Not name of *instance* of block. But it may include the
    type. E.g. `FileSource<Float>`.
     */
    fn block_name(&self) -> &'static str;

    /** Block work function

    # Args
    * `r`: Object representing all input streams to read from.
    * `w`: Object representing all output streams to write to.

    A pure Source block will not use `r`, and a pure Sink block won't
    use `w`.

    Consuming data from `r` involves first reading it, and then
    "consuming" from the stream. If a `consume()` (or `clear()`) is
    not called on the stream, the same data will continue to be read
    forever.

    Writing data to streams in `w` only involves calling `.write()` on
    the stream.
     */
    fn work(&mut self) -> Result<BlockRet, Error>;
}

/** Macro to make it easier to write one-for-one blocks.

Output type must be the same as the input type.

The first argument is the block struct name. The second (and beyond)
are traits that T must match.

`process_one(&mut self, s: &T) -> T` must be implemented by the block.

E.g.:
* [Add][add] or multiply by some constant, or negate.
* Delay, `o[n] = o[n] - o[n-1]`, or [IIR filter][iir]. These require state,
  but can process only one sample at a time.

# Example

```
use rustradio::block::Block;
use rustradio::stream::{Streamp, new_streamp};
struct Noop<T: Copy>{
  src: Streamp<T>,
  dst: Streamp<T>,
};
impl<T: Copy> Noop<T> {
  fn new(src: Streamp<T>) -> Self {
    Self {
      src,
      dst: new_streamp(),
    }
  }
  fn process_one(&self, a: &T) -> T { *a }
}
rustradio::map_block_macro_v2![Noop<T>, std::ops::Add<Output = T>];
```

[add]: ../src/rustradio/add_const.rs.html
[iir]: ../src/rustradio/single_pole_iir_filter.rs.html
*/
#[macro_export]
macro_rules! map_block_macro_v2 {
    ($name:path, $($tr:path), *) => {
        impl<T: Copy $(+$tr)*> $name {
            /// Return the output stream.
            pub fn out(&self) -> Streamp<T> {
                self.dst.clone()
            }
        }
        impl<T> $crate::block::Block for $name
        where
            T: Copy $(+$tr)*,
        {
            fn block_name(&self) -> &'static str {
                stringify!{$name}
            }
            fn work(&mut self) -> Result<$crate::block::BlockRet, $crate::Error> {
                let cs = self.src.clone();
                let mut i = cs.lock().unwrap();
                if i.is_empty() {
                    return Ok($crate::block::BlockRet::Noop);
                }
                let cd = self.dst.clone();
                cd.lock()?
                    .write(i
                           .iter()
                           .map(|x| self.process_one(x)));
                i.clear();
                Ok($crate::block::BlockRet::Ok)
            }
        }
    };
}

/** Macro to make it easier to write converting blocks.

Output type will be different from input type.

`process_one(&mut self, s: Type1) -> Type2` must be implemented by the
block.

Both types are derived, so only the name of the block is needed at
macro call.

Example block using this: `FloatToU32`.
*/
#[macro_export]
macro_rules! map_block_convert_macro {
    ($name:path) => {
        impl $crate::block::Block for $name {
            fn block_name(&self) -> &'static str {
                stringify! {$name}
            }
            fn work(&mut self) -> Result<$crate::block::BlockRet, $crate::Error> {
                let v = {
                    let c = self.src.clone();
                    let i = c.lock().unwrap();
                    if i.is_empty() {
                        return Ok($crate::block::BlockRet::Noop);
                    }
                    i.iter().map(|x| self.process_one(*x)).collect::<Vec<_>>()
                };
                self.dst.lock().unwrap().write_slice(&v);
                self.src.lock().unwrap().clear();
                Ok($crate::block::BlockRet::Ok)
            }
        }
    };
}
