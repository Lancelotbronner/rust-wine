use std::marker::PhantomData;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct TaggedPtr<T, D>(*const T, PhantomData<D>);

impl<T, D> TaggedPtr<T, D> where {
    pub const TAG_BITS: u32 = align_of::<D>().trailing_zeros();
    const TAG_MASK: usize = (1 << Self::TAG_BITS) - 1;
    const PTR_MASK: usize = !Self::TAG_MASK;

    pub fn pack(ptr: *const T, data: D) -> Self {
        println!("{}, {:#b}, {:#b}", Self::TAG_BITS, Self::TAG_MASK, Self::PTR_MASK);
        let padded = Padded(0, data, 0);
        let ptr: usize = ptr as usize;
        let data = unsafe { *(&padded.1 as *const D as *const usize) } & Self::TAG_MASK;
        let raw = (ptr | data) as *const T;
        Self(raw, PhantomData)
    }

    pub fn unpack(ptr: *const T) -> Self {
        TaggedPtr(ptr, PhantomData)
    }

    pub fn raw(&self) -> *const T {
        self.0
    }

    pub fn ptr(&self) -> *const T {
        (self.0 as usize & Self::PTR_MASK) as *const T
    }

    pub fn data(&self) -> D
    where
        D: Clone,
    {
        let tmp = self.0 as usize & Self::TAG_MASK;
        unsafe { (*(&tmp as *const usize as *const D)).clone() }
    }
}

struct Padded<T>(usize, pub T, usize);

fn test() {
    let v = 25u64;
    let ptr = TaggedPtr::pack(&v, Kind::A);
}

#[derive(Copy, Clone)]
enum Kind {
    A,
    B,
    C,
}


#[cfg(test)]
mod tests {
    use crate::ptr::TaggedPtr;

    #[test]
    fn round_trip() {
        let data = 0xdead_beef_u32;
        let packed = 2;

        let ptr = TaggedPtr::pack(&data, packed);

        assert_eq!(data, unsafe { *ptr.ptr() });
        assert_eq!(packed, ptr.data());
    }

    #[test]
    fn new() {
        let data = 0xdead_beef_u32;

        let overflow = TaggedPtr::pack(&data, 4);
        assert_ne!(overflow.data(), 4);

        let ok = TaggedPtr::pack(&data, 3);
        assert_eq!(ok.data(), 3);
    }

    #[test]
    fn eq() {
        let data = 0xdead_beef_u32;
        let ptr = TaggedPtr::pack(&data, 2);
        let ptr2 = TaggedPtr::pack(&data, 2);
        assert_eq!(ptr, ptr2);

        let ptr3 = TaggedPtr::pack(&data, 1);
        assert_ne!(ptr, ptr3);

        let data2 = 0xdead_beef_u32;
        let ptr4 = TaggedPtr::pack(&data2, 2);
        assert_ne!(ptr, ptr4);
    }
}
