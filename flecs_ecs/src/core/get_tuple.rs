#![allow(unused)]

use std::ffi::c_void;
use std::marker::PhantomData;

use crate::core::*;
use crate::sys;
use flecs_ecs_derive::tuples;
use sys::ecs_record_t;

pub struct ComponentsData<T: GetTuple, const LEN: usize> {
    pub array_components: [*mut c_void; LEN],
    pub has_all_components: bool,
    _marker: PhantomData<T>,
}

pub trait GetComponentPointers<T: GetTuple> {
    fn new<'a, const SHOULD_PANIC: bool>(
        world: impl IntoWorld<'a>,
        record: *const ecs_record_t,
    ) -> Self;

    fn get_tuple(&self) -> T::TupleType<'_>;

    fn has_all_components(&self) -> bool;
}

impl<T: GetTuple, const LEN: usize> GetComponentPointers<T> for ComponentsData<T, LEN> {
    fn new<'a, const SHOULD_PANIC: bool>(
        world: impl IntoWorld<'a>,
        record: *const ecs_record_t,
    ) -> Self {
        let mut array_components = [std::ptr::null::<c_void>() as *mut c_void; LEN];

        let has_all_components =
            T::populate_array_ptrs::<SHOULD_PANIC>(world, record, &mut array_components[..]);

        Self {
            array_components,
            has_all_components,
            _marker: PhantomData::<T>,
        }
    }

    fn get_tuple(&self) -> T::TupleType<'_> {
        T::create_tuple(&self.array_components[..])
    }

    fn has_all_components(&self) -> bool {
        self.has_all_components
    }
}

pub trait GetTupleTypeOperation {
    type ActualType;
    type OnlyType: IntoComponentId;
    const IS_OPTION: bool;
    const IS_IMMUTABLE: bool;

    fn create_tuple_data<'a>(array_components_data: *mut c_void) -> Self::ActualType;
}

impl<'w, T> GetTupleTypeOperation for &'w T
where
    T: FlecsCastType,
    T: 'w,
{
    type ActualType = &'w <T as FlecsCastType>::CastType;
    type OnlyType = T;
    const IS_OPTION: bool = false;
    const IS_IMMUTABLE: bool = true;

    fn create_tuple_data<'a>(array_components_data: *mut c_void) -> Self::ActualType {
        let data_ptr = array_components_data as *const <T as FlecsCastType>::CastType;
        // SAFETY: up to this point we have checked that the data is not null
        unsafe { &*data_ptr }
    }
}

impl<'w, T> GetTupleTypeOperation for &'w mut T
where
    T: FlecsCastType,
{
    type ActualType = &'w mut <T as FlecsCastType>::CastType;
    type OnlyType = T;
    const IS_OPTION: bool = false;
    const IS_IMMUTABLE: bool = false;

    fn create_tuple_data<'a>(array_components_data: *mut c_void) -> Self::ActualType {
        let data_ptr = array_components_data as *mut <T as FlecsCastType>::CastType;
        // SAFETY: up to this point we have checked that the data is not null
        unsafe { &mut *data_ptr }
    }
}

impl<'w, T> GetTupleTypeOperation for Option<&'w T>
where
    T: FlecsCastType,
{
    type ActualType = Option<&'w <T as FlecsCastType>::CastType>;
    type OnlyType = T;
    const IS_OPTION: bool = true;
    const IS_IMMUTABLE: bool = true;

    fn create_tuple_data<'a>(array_components_data: *mut c_void) -> Self::ActualType {
        if array_components_data.is_null() {
            None
        } else {
            let data_ptr = array_components_data as *const <T as FlecsCastType>::CastType;
            Some(unsafe { &*data_ptr })
        }
    }
}

impl<'w, T> GetTupleTypeOperation for Option<&'w mut T>
where
    T: FlecsCastType,
{
    type ActualType = Option<&'w mut <T as FlecsCastType>::CastType>;
    type OnlyType = T;
    const IS_OPTION: bool = true;
    const IS_IMMUTABLE: bool = false;

    fn create_tuple_data<'a>(array_components_data: *mut c_void) -> Self::ActualType {
        if array_components_data.is_null() {
            None
        } else {
            let data_ptr = array_components_data as *mut <T as FlecsCastType>::CastType;
            Some(unsafe { &mut *data_ptr })
        }
    }
}

pub trait GetTuple: Sized {
    type Pointers: GetComponentPointers<Self>;
    type TupleType<'a>;
    const ALL_IMMUTABLE: bool;

    fn create_ptrs<'a, const SHOULD_PANIC: bool>(
        world: impl IntoWorld<'a>,
        record: *const ecs_record_t,
    ) -> Self::Pointers {
        Self::Pointers::new::<'a, SHOULD_PANIC>(world, record)
    }

    fn populate_array_ptrs<'a, const SHOULD_PANIC: bool>(
        world: impl IntoWorld<'a>,
        record: *const ecs_record_t,
        components: &mut [*mut c_void],
    ) -> bool;

    fn create_tuple(array_components: &[*mut c_void]) -> Self::TupleType<'_>;
}

/////////////////////
// The higher sized tuples are done by a macro towards the bottom of this file.
/////////////////////

#[rustfmt::skip]
impl<A> GetTuple for A
where
    A: GetTupleTypeOperation,
{
    type Pointers = ComponentsData<A, 1>;
    type TupleType<'w> = A::ActualType;
    const ALL_IMMUTABLE: bool = A::IS_IMMUTABLE;

    fn populate_array_ptrs<'a, const SHOULD_PANIC: bool>(
        world: impl IntoWorld<'a>, record: *const ecs_record_t, components: &mut [*mut c_void]
    ) -> bool {
        let world_ptr = world.world_ptr();
        let table = unsafe { (*record).table };
        let mut has_all_components = true;

        let id = unsafe { sys::ecs_table_get_column_index(world_ptr, table, 
            <A::OnlyType as IntoComponentId>::get_id(world)) };

            if id == -1 {
                components[0] = std::ptr::null_mut();
                has_all_components = false;
                if SHOULD_PANIC && !A::IS_OPTION {
                    ecs_assert!(false, FlecsErrorCode::OperationFailed,
                        "Component `{}` not found on `EntityView::get` operation 
                        with parameters: `{}`. 
                        Use `try_get` variant to avoid assert/panicking if you want to handle the error 
                        or use `Option<{}> instead to handle individual cases.",
                        std::any::type_name::<A::OnlyType>(), std::any::type_name::<Self>(), std::any::type_name::<A::ActualType>());
                    panic!("Component `{}` not found on `EntityView::get` operation 
                    with parameters: `{}`. 
                    Use `try_get` variant to avoid assert/panicking if 
                    you want to handle the error or use `Option<{}> 
                    instead to handle individual cases.",
                    std::any::type_name::<A::OnlyType>(), std::any::type_name::<Self>(), std::any::type_name::<A::ActualType>());
                }
            } else { 
                components[0] = unsafe { sys::ecs_record_get_column(record, id, 0) };
            } 
        

        has_all_components
    }

    fn create_tuple(array_components: &[*mut c_void]) -> Self::TupleType<'_> {
        A::create_tuple_data(array_components[0])
    }
}
pub struct Wrapper<T>(T);

pub trait TupleForm<'a, T, U> {
    type Tuple;
    const IS_OPTION: bool;

    fn return_type_for_tuple(array: *mut U, index: usize) -> Self::Tuple;
}

impl<'a, T: 'a> TupleForm<'a, T, T> for Wrapper<T> {
    type Tuple = &'a mut T;
    const IS_OPTION: bool = false;

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    #[inline(always)]
    fn return_type_for_tuple(array: *mut T, index: usize) -> Self::Tuple {
        unsafe { &mut (*array.add(index)) }
    }
}

impl<'a, T: 'a> TupleForm<'a, Option<T>, T> for Wrapper<T> {
    type Tuple = Option<&'a mut T>;
    const IS_OPTION: bool = true;

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    #[inline(always)]
    fn return_type_for_tuple(array: *mut T, index: usize) -> Self::Tuple {
        unsafe {
            if array.is_null() {
                None
            } else {
                Some(&mut (*array.add(index)))
            }
        }
    }
}

macro_rules! tuple_count {
    () => { 0 };
    ($head:ident) => { 1 };
    ($head:ident, $($tail:ident),*) => { 1 + tuple_count!($($tail),*) };
}

macro_rules! impl_get_tuple {
    ($($t:ident),*) => {
        impl<$($t: GetTupleTypeOperation),*> GetTuple for ($($t,)*) {
            type TupleType<'w> = ($(
                $t::ActualType,
            )*);

            type Pointers = ComponentsData<Self, { tuple_count!($($t),*) }>;

            const ALL_IMMUTABLE: bool = { $($t::IS_IMMUTABLE &&)* true };

            #[allow(unused)]
            fn populate_array_ptrs<'a, const SHOULD_PANIC: bool>(
                world: impl IntoWorld<'a>, record: *const ecs_record_t, components: &mut [*mut c_void]
            ) -> bool {

                let world_ptr = world.world_ptr();
                let world_ref = world.world();
                let table = unsafe { (*record).table };
                let mut index : usize = 0;
                let mut has_all_components = true;

                $(
                    let column_index = unsafe { sys::ecs_table_get_column_index(world_ptr, table,
                        <$t::OnlyType as IntoComponentId>::get_id(world_ref)) };


                    if column_index != -1 {
                        components[index] = unsafe { sys::ecs_record_get_column(record, column_index, 0) };
                    } else {
                        components[index] = std::ptr::null_mut();
                        if !$t::IS_OPTION {
                            if SHOULD_PANIC {
                                // ecs_assert!(false, FlecsErrorCode::OperationFailed,
                                //     "Component `{}` not found on `EntityView::get` operation
                                //     with parameters: `{}`.
                                //     Use `try_get` variant to avoid assert/panicking if you want to handle
                                //     the error or use `Option<{}> instead to handle individual cases.",
                                //     std::any::type_name::<$t::OnlyType>(), std::any::type_name::<Self>(),
                                //     std::any::type_name::<$t::ActualType>());
                                panic!("Component `{}` not found on `EntityView::get`operation 
                                with parameters: `{}`. 
                                Use `try_get` variant to avoid assert/panicking if you want to handle the error 
                                or use `Option<{}> instead to handle individual cases.", std::any::type_name::<$t::OnlyType>(),
                                std::any::type_name::<Self>(), std::any::type_name::<$t::ActualType>());
                            }
                            has_all_components = false;
                        }
                    }
                    index += 1;
                )*

                has_all_components
            }

            #[allow(unused, clippy::unused_unit)]
            fn create_tuple(array_components: &[*mut c_void]) -> Self::TupleType<'_> {
                let mut column: isize = -1;
                ($({
                    column += 1;
                    $t::create_tuple_data(array_components[column as usize])
                },)*)
            }

        }
    }
}

tuples!(impl_get_tuple, 0, 12);

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    #[derive(Component)]
    struct Position {
        x: f32,
        y: f32,
    }

    #[derive(Component)]
    struct Velocity {
        dx: f32,
        dy: f32,
    }

    #[test]
    fn are_all_terms_const() {
        assert_eq!(<(&Position, &Velocity) as GetTuple>::ALL_IMMUTABLE, true);

        assert_eq!(
            <(Option<&Position>, &Velocity) as GetTuple>::ALL_IMMUTABLE,
            true
        );

        assert_eq!(
            <(&Position, Option<&Velocity>) as GetTuple>::ALL_IMMUTABLE,
            true
        );

        assert_eq!(
            <(Option<&Position>, Option<&Velocity>) as GetTuple>::ALL_IMMUTABLE,
            true
        );

        assert_eq!(
            <(&mut Position, &Velocity) as GetTuple>::ALL_IMMUTABLE,
            false
        );

        assert_eq!(
            <(&Position, &mut Velocity) as GetTuple>::ALL_IMMUTABLE,
            false
        );

        assert_eq!(
            <(Option<&mut Position>, &Velocity) as GetTuple>::ALL_IMMUTABLE,
            false
        );

        assert_eq!(
            <(Option<&Position>, &mut Velocity) as GetTuple>::ALL_IMMUTABLE,
            false
        );

        assert_eq!(
            <(&mut Position, Option<&Velocity>) as GetTuple>::ALL_IMMUTABLE,
            false
        );

        assert_eq!(
            <(Option<&mut Position>, Option<&Velocity>) as GetTuple>::ALL_IMMUTABLE,
            false
        );

        assert_eq!(
            <(&mut Position, &mut Velocity) as GetTuple>::ALL_IMMUTABLE,
            false
        );

        assert_eq!(
            <(Option<&mut Position>, &mut Velocity) as GetTuple>::ALL_IMMUTABLE,
            false
        );
    }
}
