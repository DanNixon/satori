mod creation;
pub(super) use creation::*;

mod deletion;
pub(super) use deletion::*;

mod misc;
pub(super) use misc::*;

mod retrieval;
pub(super) use retrieval::*;

macro_rules! all_storage_tests {
    ( $test_macro:ident ) => {
        $test_macro!(test_add_first_event);
        $test_macro!(test_add_event);
        $test_macro!(test_add_segment_new_camera);
        $test_macro!(test_add_segment_existing_camera);

        $test_macro!(test_delete_event);
        $test_macro!(test_delete_event_filename);
        $test_macro!(test_delete_segment);
        $test_macro!(test_delete_last_segment_deletes_camera);

        $test_macro!(test_init);

        $test_macro!(test_event_getters);
        $test_macro!(test_segment_getters);
    };
}

pub(super) use all_storage_tests;
