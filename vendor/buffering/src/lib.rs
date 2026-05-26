#![deny(missing_docs)]

//! # Approach
//! This crate is meant to provide two methods for serializing and deserializing with buffer
//! operations:
//! * The `copy` module is a more flexible, high-level stream based approach for reading from and
//! writing to buffers. This will introduce some overhead so do not use this if copies are a bottleneck.
//! * The `nocopy` module is a more restrictive macro-based approach. It uses procedural macros and
//! unions to provide some level of safety when writing to fields in a struct while allowing the
//! underlying struct to be interpreted as a slice. This is a C-like workflow but does provide
//! some helpful guarantees that come with Rust like protection against buffer overflows and
//! bounds checking. One very important note is that structs that use the provided
//! procedural macro must be completely stack allocated. Compilation will fail if certain
//! constructs that prevent size computation at compile time are used. As a result this really
//! should only be used as a Rust substitute with some additional safety for the common C workflow
//! when doing things like parsing network packets.
//!
//! # Features
//! Each module is feature-flagged so that either or both can be used.  Available features are:
//! * `copy`
//! * `nocopy`

#[cfg(feature = "nocopy")]
#[allow(unused_imports)]
extern crate buffering_nocopy_macro;

#[cfg(feature = "nocopy")]
pub use buffering_nocopy_macro::NoCopy;

/// A more flexible way to serialize and deserialize into buffers that will have some copy overhead
#[cfg(feature = "copy")]
mod copy;

#[cfg(feature = "copy")]
pub use copy::*;

#[cfg(all(test, feature = "nocopy"))]
mod test {
    use super::*;

    #[test]
    fn test_proc_macro() {
        #[derive(Copy, Clone, NoCopy)]
        #[repr(C)]
        #[nocopy_macro(name = "TestBufferThing")]
        pub struct Test {
            test: u8,
        }

        let mut tb = TestBufferThing::new_buffer([0]);
        tb.set_test(5);
        assert_eq!(tb.as_buffer(), [5]);

        #[derive(Copy, Clone, NoCopy)]
        #[repr(C)]
        #[nocopy_macro(name = "TestBufferThingTwo", endian = "big")]
        pub struct TestTwo {
            test: u16,
        }

        let tb = TestBufferThingTwo::new_buffer([0, 5]);
        assert_eq!(tb.get_test(), 5);

        #[derive(Copy, Clone, NoCopy)]
        #[repr(C)]
        #[nocopy_macro(name = "TestBufferThingThree", endian = "little")]
        pub struct TestThree {
            test: u16,
        }

        let tb = TestBufferThingThree::new_buffer([5, 0]);
        assert_eq!(tb.get_test(), 5);
    }
}
