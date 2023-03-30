use crate::{
    de::{read::Reader, BorrowDecoder, Decode, Decoder},
    enc::{
        self,
        write::{SizeWriter, Writer},
        Encode, Encoder,
    },
    error::{DecodeError, EncodeError},
    impl_borrow_decode, BorrowDecode, Config,
};
#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc;
use alloc::{
    borrow::{Cow, ToOwned},
    boxed::Box,
    rc::Rc,
    string::String,
    vec::Vec,
};

#[derive(Default)]
pub(crate) struct VecWriter {
    inner: Vec<u8>,
}

impl VecWriter {
    /// Create a new vec writer with the given capacity
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            inner: Vec::with_capacity(cap),
        }
    }
    // May not be used in all feature combinations
    #[allow(dead_code)]
    pub(crate) fn collect(self) -> Vec<u8> {
        self.inner
    }
}

impl enc::write::Writer for VecWriter {
    #[inline(always)]
    fn write(&mut self, bytes: &[u8]) -> Result<(), EncodeError> {
        self.inner.try_reserve(bytes.len())?;

        let start = self.inner.len();

        // Get a slice of `&mut [MaybeUninit<u8>]` of the remaining capacity
        let remaining = &mut self.inner.spare_capacity_mut()[..bytes.len()];
        for (i, b) in bytes.iter().copied().enumerate() {
            // TODO: is there a better way to copy from `&mut [MaybeUninit<u8>]` to `&[u8]`?
            remaining[i].write(b);
        }

        unsafe {
            // Safety: We reserved enough bytes, and the bytes have values written to them
            self.inner.set_len(start + bytes.len());
        }
        Ok(())
    }
}

/// Encode the given value into a `Vec<u8>` with the given `Config`. See the [config] module for more information.
///
/// [config]: config/index.html
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub fn encode_to_vec<E: enc::Encode, C: Config>(val: E, config: C) -> Result<Vec<u8>, EncodeError> {
    let size = {
        let mut size_writer = enc::EncoderImpl::<_, C>::new(SizeWriter::default(), config);
        val.encode(&mut size_writer)?;
        size_writer.into_writer().bytes_written
    };
    let writer = VecWriter::with_capacity(size);
    let mut encoder = enc::EncoderImpl::<_, C>::new(writer, config);
    val.encode(&mut encoder)?;
    Ok(encoder.into_writer().inner)
}

// TODO: these collections straight up don't exist with `no_global_oom_handling`
#[cfg(not(feature = "unstable-strict-oom-checks"))]
mod collections {
    use super::*;
    use alloc::collections::{BTreeMap, BTreeSet, BinaryHeap, VecDeque};

    impl<T> Decode for BinaryHeap<T>
    where
        T: Decode + Ord,
    {
        fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
            let len = crate::de::decode_slice_len(decoder)?;
            decoder.claim_container_read::<T>(len)?;

            let mut map = BinaryHeap::new();
            map.try_reserve(len)?;

            for _ in 0..len {
                // See the documentation on `unclaim_bytes_read` as to why we're doing this here
                decoder.unclaim_bytes_read(core::mem::size_of::<T>());

                let key = T::decode(decoder)?;
                map.push(key);
            }
            Ok(map)
        }
    }
    impl<'de, T> BorrowDecode<'de> for BinaryHeap<T>
    where
        T: BorrowDecode<'de> + Ord,
    {
        fn borrow_decode<D: BorrowDecoder<'de>>(decoder: &mut D) -> Result<Self, DecodeError> {
            let len = crate::de::decode_slice_len(decoder)?;
            decoder.claim_container_read::<T>(len)?;

            let mut map = BinaryHeap::new();
            map.try_reserve(len)?;
            for _ in 0..len {
                // See the documentation on `unclaim_bytes_read` as to why we're doing this here
                decoder.unclaim_bytes_read(core::mem::size_of::<T>());

                let key = T::borrow_decode(decoder)?;
                map.push(key);
            }
            Ok(map)
        }
    }

    impl<T> Encode for BinaryHeap<T>
    where
        T: Encode + Ord,
    {
        fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
            crate::enc::encode_slice_len(encoder, self.len())?;
            for val in self.iter() {
                val.encode(encoder)?;
            }
            Ok(())
        }
    }

    impl<K, V> Decode for BTreeMap<K, V>
    where
        K: Decode + Ord,
        V: Decode,
    {
        fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
            let len = crate::de::decode_slice_len(decoder)?;
            decoder.claim_container_read::<(K, V)>(len)?;

            let mut map = BTreeMap::new();
            for _ in 0..len {
                // See the documentation on `unclaim_bytes_read` as to why we're doing this here
                decoder.unclaim_bytes_read(core::mem::size_of::<(K, V)>());

                let key = K::decode(decoder)?;
                let value = V::decode(decoder)?;
                map.insert(key, value);
            }
            Ok(map)
        }
    }
    impl<'de, K, V> BorrowDecode<'de> for BTreeMap<K, V>
    where
        K: BorrowDecode<'de> + Ord,
        V: BorrowDecode<'de>,
    {
        fn borrow_decode<D: BorrowDecoder<'de>>(decoder: &mut D) -> Result<Self, DecodeError> {
            let len = crate::de::decode_slice_len(decoder)?;
            decoder.claim_container_read::<(K, V)>(len)?;

            let mut map = BTreeMap::new();
            for _ in 0..len {
                // See the documentation on `unclaim_bytes_read` as to why we're doing this here
                decoder.unclaim_bytes_read(core::mem::size_of::<(K, V)>());

                let key = K::borrow_decode(decoder)?;
                let value = V::borrow_decode(decoder)?;
                map.insert(key, value);
            }
            Ok(map)
        }
    }

    impl<K, V> Encode for BTreeMap<K, V>
    where
        K: Encode + Ord,
        V: Encode,
    {
        fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
            crate::enc::encode_slice_len(encoder, self.len())?;
            for (key, val) in self.iter() {
                key.encode(encoder)?;
                val.encode(encoder)?;
            }
            Ok(())
        }
    }

    impl<T> Decode for BTreeSet<T>
    where
        T: Decode + Ord,
    {
        fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
            let len = crate::de::decode_slice_len(decoder)?;
            decoder.claim_container_read::<T>(len)?;

            let mut map = BTreeSet::new();
            for _ in 0..len {
                // See the documentation on `unclaim_bytes_read` as to why we're doing this here
                decoder.unclaim_bytes_read(core::mem::size_of::<T>());

                let key = T::decode(decoder)?;
                map.insert(key);
            }
            Ok(map)
        }
    }
    impl<'de, T> BorrowDecode<'de> for BTreeSet<T>
    where
        T: BorrowDecode<'de> + Ord,
    {
        fn borrow_decode<D: BorrowDecoder<'de>>(decoder: &mut D) -> Result<Self, DecodeError> {
            let len = crate::de::decode_slice_len(decoder)?;
            decoder.claim_container_read::<T>(len)?;

            let mut map = BTreeSet::new();
            for _ in 0..len {
                // See the documentation on `unclaim_bytes_read` as to why we're doing this here
                decoder.unclaim_bytes_read(core::mem::size_of::<T>());

                let key = T::borrow_decode(decoder)?;
                map.insert(key);
            }
            Ok(map)
        }
    }

    impl<T> Encode for BTreeSet<T>
    where
        T: Encode + Ord,
    {
        fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
            crate::enc::encode_slice_len(encoder, self.len())?;
            for item in self.iter() {
                item.encode(encoder)?;
            }
            Ok(())
        }
    }

    impl<T> Decode for VecDeque<T>
    where
        T: Decode,
    {
        fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
            let len = crate::de::decode_slice_len(decoder)?;
            decoder.claim_container_read::<T>(len)?;

            let mut map = VecDeque::new();
            map.try_reserve(len)?;

            for _ in 0..len {
                // See the documentation on `unclaim_bytes_read` as to why we're doing this here
                decoder.unclaim_bytes_read(core::mem::size_of::<T>());

                let key = T::decode(decoder)?;
                map.push_back(key);
            }
            Ok(map)
        }
    }
    impl<'de, T> BorrowDecode<'de> for VecDeque<T>
    where
        T: BorrowDecode<'de>,
    {
        fn borrow_decode<D: BorrowDecoder<'de>>(decoder: &mut D) -> Result<Self, DecodeError> {
            let len = crate::de::decode_slice_len(decoder)?;
            decoder.claim_container_read::<T>(len)?;

            let mut map = VecDeque::new();
            map.try_reserve(len)?;
            for _ in 0..len {
                // See the documentation on `unclaim_bytes_read` as to why we're doing this here
                decoder.unclaim_bytes_read(core::mem::size_of::<T>());

                let key = T::borrow_decode(decoder)?;
                map.push_back(key);
            }
            Ok(map)
        }
    }

    impl<T> Encode for VecDeque<T>
    where
        T: Encode,
    {
        fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
            crate::enc::encode_slice_len(encoder, self.len())?;
            for item in self.iter() {
                item.encode(encoder)?;
            }
            Ok(())
        }
    }
}

impl<T> Decode for Vec<T>
where
    T: Decode + 'static,
{
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let len = crate::de::decode_slice_len(decoder)?;

        if core::any::TypeId::of::<T>() == core::any::TypeId::of::<u8>() {
            decoder.claim_container_read::<T>(len)?;
            // optimize for reading u8 vecs
            let mut vec = Vec::new();
            vec.resize(len, 0u8);
            decoder.reader().read(&mut vec)?;
            // Safety: Vec<T> is Vec<u8>
            return Ok(unsafe { core::mem::transmute(vec) });
        }
        decoder.claim_container_read::<T>(len)?;

        let mut vec = Vec::new();
        vec.try_reserve(len)?;

        let slice = vec.spare_capacity_mut();
        let mut guard = DropGuard { slice, idx: 0 };

        for _ in 0..len {
            // See the documentation on `unclaim_bytes_read` as to why we're doing this here
            decoder.unclaim_bytes_read(core::mem::size_of::<T>());

            let t = T::decode(decoder)?;
            guard.slice[guard.idx].write(t);
            guard.idx += 1;
        }
        // Don't drop the guard
        core::mem::forget(guard);
        unsafe {
            // All values are written, we can now set the length of the vec
            vec.set_len(vec.len() + len)
        }
        Ok(vec)
    }
}

impl<'de, T> BorrowDecode<'de> for Vec<T>
where
    T: BorrowDecode<'de>,
{
    fn borrow_decode<D: BorrowDecoder<'de>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let len = crate::de::decode_slice_len(decoder)?;
        decoder.claim_container_read::<T>(len)?;

        let mut vec = Vec::new();
        vec.try_reserve(len)?;

        let slice = vec.spare_capacity_mut();
        let mut guard = DropGuard { slice, idx: 0 };

        for _ in 0..len {
            // See the documentation on `unclaim_bytes_read` as to why we're doing this here
            decoder.unclaim_bytes_read(core::mem::size_of::<T>());

            let t = T::borrow_decode(decoder)?;
            guard.slice[guard.idx].write(t);
            guard.idx += 1;
        }
        // Don't drop the guard
        core::mem::forget(guard);
        unsafe {
            // All values are written, we can now set the length of the vec
            vec.set_len(vec.len() + len)
        }
        Ok(vec)
    }
}

impl<T> Encode for Vec<T>
where
    T: Encode + 'static,
{
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        crate::enc::encode_slice_len(encoder, self.len())?;
        if core::any::TypeId::of::<T>() == core::any::TypeId::of::<u8>() {
            let slice: &[u8] = unsafe { core::mem::transmute(self.as_slice()) };
            encoder.writer().write(slice)?;
            return Ok(());
        }
        for item in self.iter() {
            item.encode(encoder)?;
        }
        Ok(())
    }
}

impl Decode for String {
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let bytes = Vec::<u8>::decode(decoder)?;
        String::from_utf8(bytes).map_err(|e| DecodeError::Utf8 {
            inner: e.utf8_error(),
        })
    }
}
impl_borrow_decode!(String);

// TODO
// String does not implement Into for Box<str> because it allocates again
// we could do this manually with `Box::try_new_uninit`
#[cfg(not(feature = "unstable-strict-oom-checks"))]
impl Decode for Box<str> {
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        String::decode(decoder).map(String::into_boxed_str)
    }
}

// TODO
// String does not implement Into for Box<str> because it allocates again
// we could do this manually with `Box::try_new_uninit`
#[cfg(not(feature = "unstable-strict-oom-checks"))]
impl_borrow_decode!(Box<str>);

impl Encode for String {
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        self.as_bytes().encode(encoder)
    }
}

impl<T> Decode for Box<T>
where
    T: Decode,
{
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let t = T::decode(decoder)?;
        #[cfg(feature = "unstable-strict-oom-checks")]
        let b = Box::try_new(t)?;
        #[cfg(not(feature = "unstable-strict-oom-checks"))]
        let b = Box::new(t);
        Ok(b)
    }
}
impl<'de, T> BorrowDecode<'de> for Box<T>
where
    T: BorrowDecode<'de>,
{
    fn borrow_decode<D: BorrowDecoder<'de>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let t = T::borrow_decode(decoder)?;
        #[cfg(feature = "unstable-strict-oom-checks")]
        let b = Box::try_new(t)?;
        #[cfg(not(feature = "unstable-strict-oom-checks"))]
        let b = Box::new(t);
        Ok(b)
    }
}

impl<T> Encode for Box<T>
where
    T: Encode + ?Sized,
{
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        T::encode(self, encoder)
    }
}

#[cfg(feature = "unstable-strict-oom-checks")]
impl<T> Decode for Box<[T]>
where
    T: Decode + 'static,
{
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let len = crate::de::decode_slice_len(decoder)?;
        decoder.claim_container_read::<T>(len)?;

        unsafe {
            let mut result = Box::try_new_uninit_slice(len)?;

            let mut guard = DropGuard {
                slice: &mut result,
                idx: 0,
            };

            while guard.idx < len {
                decoder.unclaim_bytes_read(core::mem::size_of::<T>());
                let t = T::decode(decoder)?;

                guard.slice.get_unchecked_mut(guard.idx).write(t);
                guard.idx += 1;
            }

            core::mem::forget(guard);
            Ok(result.assume_init())
        }
    }
}

#[cfg(not(feature = "unstable-strict-oom-checks"))]
impl<T> Decode for Box<[T]>
where
    T: Decode,
{
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let vec = Vec::<T>::decode(decoder)?;
        Ok(vec.into())
    }
}

// TODO
// Vec does not implement Into for Box<[T]> because it allocates again
// we could do this manually with `Box::try_new_uninit`
#[cfg(not(feature = "unstable-strict-oom-checks"))]
impl<'de, T> BorrowDecode<'de> for Box<[T]>
where
    T: BorrowDecode<'de> + 'de,
{
    fn borrow_decode<D: BorrowDecoder<'de>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let vec = Vec::borrow_decode(decoder)?;
        Ok(vec.into_boxed_slice())
    }
}

impl<'cow, T> Decode for Cow<'cow, T>
where
    T: ToOwned + ?Sized,
    <T as ToOwned>::Owned: Decode,
{
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let t = <T as ToOwned>::Owned::decode(decoder)?;
        Ok(Cow::Owned(t))
    }
}
impl<'cow, T> BorrowDecode<'cow> for Cow<'cow, T>
where
    T: ToOwned + ?Sized,
    &'cow T: BorrowDecode<'cow>,
{
    fn borrow_decode<D: BorrowDecoder<'cow>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let t = <&T>::borrow_decode(decoder)?;
        Ok(Cow::Borrowed(t))
    }
}

impl<'cow, T> Encode for Cow<'cow, T>
where
    T: ToOwned + ?Sized,
    for<'a> &'a T: Encode,
{
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        self.as_ref().encode(encoder)
    }
}

#[test]
fn test_cow_round_trip() {
    let start = Cow::Borrowed("Foo");
    let encoded = crate::encode_to_vec(&start, crate::config::standard()).unwrap();
    let (end, _) =
        crate::borrow_decode_from_slice::<Cow<str>, _>(&encoded, crate::config::standard())
            .unwrap();
    assert_eq!(start, end);
    let (end, _) =
        crate::decode_from_slice::<Cow<str>, _>(&encoded, crate::config::standard()).unwrap();
    assert_eq!(start, end);
}

impl<T> Decode for Rc<T>
where
    T: Decode,
{
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let t = T::decode(decoder)?;
        #[cfg(feature = "unstable-strict-oom-checks")]
        let rc = Rc::try_new(t)?;
        #[cfg(not(feature = "unstable-strict-oom-checks"))]
        let rc = Rc::new(t);
        Ok(rc)
    }
}

impl<'de, T> BorrowDecode<'de> for Rc<T>
where
    T: BorrowDecode<'de>,
{
    fn borrow_decode<D: BorrowDecoder<'de>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let t = T::borrow_decode(decoder)?;
        #[cfg(feature = "unstable-strict-oom-checks")]
        let rc = Rc::try_new(t)?;
        #[cfg(not(feature = "unstable-strict-oom-checks"))]
        let rc = Rc::new(t);
        Ok(rc)
    }
}

impl<T> Encode for Rc<T>
where
    T: Encode + ?Sized,
{
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        T::encode(self, encoder)
    }
}

// TODO
// Vec does not implement Into for Rc<[T]> because it allocates again
// we could do this manually with `Rc::try_new_uninit`
#[cfg(not(feature = "unstable-strict-oom-checks"))]
impl<T> Decode for Rc<[T]>
where
    T: Decode + 'static,
{
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let vec = Vec::decode(decoder)?;
        Ok(vec.into())
    }
}

// TODO
// Vec does not implement Into for Rc<[T]> because it allocates again
// we could do this manually with `Rc::try_new_uninit`
#[cfg(not(feature = "unstable-strict-oom-checks"))]
impl<'de, T> BorrowDecode<'de> for Rc<[T]>
where
    T: BorrowDecode<'de> + 'de,
{
    fn borrow_decode<D: BorrowDecoder<'de>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let vec = Vec::borrow_decode(decoder)?;
        Ok(vec.into())
    }
}

#[cfg(target_has_atomic = "ptr")]
impl<T> Decode for Arc<T>
where
    T: Decode,
{
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let t = T::decode(decoder)?;
        #[cfg(feature = "unstable-strict-oom-checks")]
        let arc = Arc::try_new(t)?;
        #[cfg(not(feature = "unstable-strict-oom-checks"))]
        let arc = Arc::new(t);
        Ok(arc)
    }
}

// TODO
// String does not implement Into for Arc<str> because it allocates again
// we could do this manually with `Arc::try_new_uninit`
#[cfg(not(feature = "unstable-strict-oom-checks"))]
#[cfg(target_has_atomic = "ptr")]
impl Decode for Arc<str> {
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let decoded = String::decode(decoder)?;
        Ok(decoded.into())
    }
}

#[cfg(target_has_atomic = "ptr")]
impl<'de, T> BorrowDecode<'de> for Arc<T>
where
    T: BorrowDecode<'de>,
{
    fn borrow_decode<D: BorrowDecoder<'de>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let t = T::borrow_decode(decoder)?;
        #[cfg(feature = "unstable-strict-oom-checks")]
        let arc = Arc::try_new(t)?;
        #[cfg(not(feature = "unstable-strict-oom-checks"))]
        let arc = Arc::new(t);
        Ok(arc)
    }
}

// TODO
// String does not implement Into for Arc<str> because it allocates again
// we could do this manually with `Arc::try_new_uninit`
#[cfg(not(feature = "unstable-strict-oom-checks"))]
#[cfg(target_has_atomic = "ptr")]
impl<'de> BorrowDecode<'de> for Arc<str> {
    fn borrow_decode<D: BorrowDecoder<'de>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let decoded = String::decode(decoder)?;
        Ok(decoded.into())
    }
}

#[cfg(target_has_atomic = "ptr")]
impl<T> Encode for Arc<T>
where
    T: Encode + ?Sized,
{
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        T::encode(self, encoder)
    }
}

// TODO
// Vec<T> does not implement Into for Arc<[T]>
// we could do this manually with `Arc::try_new_uninit`
#[cfg(not(feature = "unstable-strict-oom-checks"))]
#[cfg(target_has_atomic = "ptr")]
impl<T> Decode for Arc<[T]>
where
    T: Decode + 'static,
{
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let vec = Vec::decode(decoder)?;
        Ok(vec.into())
    }
}

// TODO
// Vec<T> does not implement Into for Arc<[T]>
// we could do this manually with `Arc::try_new_uninit`
#[cfg(not(feature = "unstable-strict-oom-checks"))]
#[cfg(target_has_atomic = "ptr")]
impl<'de, T> BorrowDecode<'de> for Arc<[T]>
where
    T: BorrowDecode<'de> + 'de,
{
    fn borrow_decode<D: BorrowDecoder<'de>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let vec = Vec::borrow_decode(decoder)?;
        Ok(vec.into())
    }
}

/// A drop guard that will trigger when an item fails to decode.
/// If an item at index n fails to decode, we have to properly drop the 0..(n-1) values that have been read.
struct DropGuard<'a, T> {
    slice: &'a mut [core::mem::MaybeUninit<T>],
    idx: usize,
}

impl<'a, T> Drop for DropGuard<'a, T> {
    fn drop(&mut self) {
        unsafe {
            for item in &mut self.slice[..self.idx] {
                core::ptr::drop_in_place(item as *mut core::mem::MaybeUninit<T> as *mut T);
            }
        }
    }
}
