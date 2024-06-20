#![doc = include_str!("../Readme.md")]

use libxml::bindings::{
    xmlC14NDocDumpMemory, xmlChar, xmlDocPtr, xmlFree, xmlFreeDoc, xmlNodeSet, xmlReadDoc,
};
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::iter::once;
use std::ptr::null;
use thiserror::Error;

/// Options for configuring how to canonicalize XML
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
pub struct CanonicalizationOptions {
    pub mode: CanonicalizationMode,
    /// If true, keep `<!-- ... -->` comments, otherwise remove
    pub keep_comments: bool,
    /// Namespaces to keep even if they are unused. By default, in [CanonicalizationMode::ExclusiveCanonical1_0], unused namespaces are removed.
    ///
    /// Doesn't apply to other canonicalization modes.
    pub inclusive_ns_prefixes: Vec<String>,
}

/// Canonicalization specification to use
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
pub enum CanonicalizationMode {
    /// Original C14N 1.0 spec
    Canonical1_0,
    /// Exclusive C14N 1.0 spec
    #[default]
    ExclusiveCanonical1_0,
    /// C14N 1.1 spec
    Canonical1_1,
}

impl CanonicalizationMode {
    fn to_c_int(self) -> c_int {
        c_int::from(match self {
            CanonicalizationMode::Canonical1_0 => 0,
            CanonicalizationMode::ExclusiveCanonical1_0 => 1,
            CanonicalizationMode::Canonical1_1 => 2,
        })
    }
}

/// An error code (always negative) returned by libxml2 when attempting to canonicalize some XML
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default, Error)]
#[error("canonicalization error ({0})")]
pub struct CanonicalizationErrorCode(i32);

/// Parse specified XML document and canonicalize it
///
/// Example:
///
/// ```
/// use xml_c14n::{canonicalize_xml, CanonicalizationOptions, CanonicalizationMode};
///
/// let canonicalized = canonicalize_xml(
///     "<hi/>",
///     CanonicalizationOptions {
///         mode: CanonicalizationMode::Canonical1_0,
///         keep_comments: false,
///         inclusive_ns_prefixes: vec![],
///     }
/// ).unwrap();
///
/// assert_eq!(canonicalized, "<hi></hi>")
/// ```
pub fn canonicalize_xml(
    document: &str,
    options: CanonicalizationOptions,
) -> Result<String, CanonicalizationErrorCode> {
    // not sure how this works, but if XML is valid, this still succeeds, but canonicalize_document_to_c_pointer fails below.
    let document = read_document(document);

    unsafe {
        let (output, return_code) = canonicalize_document_to_c_pointer(options, document);

        let result = if return_code < 0 {
            Err(CanonicalizationErrorCode(return_code))
        } else {
            // SAFETY: xmlC14NDocDumpMemory completed successfully, so a proper C string was allocated and assigned to `output`
            let c_str = CStr::from_ptr(output as *const _);
            let str_slice: &str = c_str.to_str().unwrap();
            Ok(str_slice.to_owned())
        };

        // let free_function = xmlFree.unwrap();
        // free_function(output as *mut c_void);

        xmlFreeDoc(document);

        result
    }
}

/// Canonicalize document
///
/// If the operation completes successfully (return code is not negative), the returned pointer points to a valid C String
unsafe fn canonicalize_document_to_c_pointer(
    options: CanonicalizationOptions,
    document: xmlDocPtr,
) -> (*const xmlChar, c_int) {
    // "NULL if all document nodes should be included"
    let nodes = null::<xmlNodeSet>() as *mut _;

    let mut ns_list_c = to_xml_string_vec(options.inclusive_ns_prefixes);
    let with_comments = c_int::from(options.keep_comments);

    let mut output = null::<xmlChar>() as *mut xmlChar;

    let return_code = xmlC14NDocDumpMemory(
        document,
        nodes,
        options.mode.to_c_int(),
        ns_list_c.as_mut_ptr(),
        with_comments,
        (&mut output) as *mut _,
    );

    free_xml_string_vec(ns_list_c);

    (output, return_code)
}

/// Create a [Vec] of null-terminated [*mut xmlChar] strings
fn to_xml_string_vec(vec: Vec<String>) -> Vec<*mut xmlChar> {
    vec.into_iter()
        .map(|s| CString::new(s).unwrap().into_raw() as *mut xmlChar)
        .chain(once(std::ptr::null_mut()))
        .collect()
}

/// Deallocate a [Vec] of null-terminated [*mut xmlChar] strings
///
/// # Safety
///
/// The argument must have been created with [to_xml_string_vec]
unsafe fn free_xml_string_vec(vec: Vec<*mut xmlChar>) {
    for s in vec {
        if !s.is_null() {
            let _ = CString::from_raw(s as *mut c_char);
        }
    }
}

/// Parse the specified string to a [xmlDocPtr]
fn read_document(document: &str) -> xmlDocPtr {
    unsafe {
        let c_document = CString::new(document).unwrap();
        // TODO...
        let url = CString::default();

        // Encoding is allowed to be null as per docs
        let encoding = null();

        xmlReadDoc(
            // Pointer to null-terminated UTF8 required, which is what we pass here
            c_document.as_ptr() as *const xmlChar,
            url.as_ptr(),
            encoding,
            c_int::from(0),
        )
    }
}

#[cfg(test)]
mod tests {
    //! Test cases are taken from official spec and other sources. For more info see corresponding Readmes.
    use super::*;

    #[test]
    fn canonical_1_1_example_3_1_no_comment() {
        let input = include_str!("samples/canonical_1_1/3_1_input.xml");
        let expected = include_str!("samples/canonical_1_1/3_1_output_no_comment.xml");

        let canonicalized = canonicalize_xml(
            input,
            CanonicalizationOptions {
                mode: CanonicalizationMode::Canonical1_1,
                keep_comments: false,
                inclusive_ns_prefixes: vec![],
            },
        )
        .unwrap();
        assert_eq!(canonicalized, expected)
    }

    #[test]
    fn canonical_1_1_example_3_1() {
        let input = include_str!("samples/canonical_1_1/3_1_input.xml");
        let expected = include_str!("samples/canonical_1_1/3_1_output.xml");

        let canonicalized = canonicalize_xml(
            input,
            CanonicalizationOptions {
                mode: CanonicalizationMode::Canonical1_1,
                keep_comments: true,
                inclusive_ns_prefixes: vec![],
            },
        )
        .unwrap();
        assert_eq!(canonicalized, expected)
    }

    #[test]
    fn canonical_1_1_example_3_2() {
        let input = include_str!("samples/canonical_1_1/3_2_input.xml");
        let expected = include_str!("samples/canonical_1_1/3_2_output.xml");

        let canonicalized = canonicalize_xml(
            input,
            CanonicalizationOptions {
                mode: CanonicalizationMode::Canonical1_1,
                keep_comments: true,
                inclusive_ns_prefixes: vec![],
            },
        )
        .unwrap();

        // for some reason, we get a stray \n at end of file :/
        assert_eq!(canonicalized, expected.trim())
    }

    #[test]
    fn canonical_exclusive_example_1() {
        let input = include_str!("samples/canonical_exclusive/1_input.xml");
        let expected = include_str!("samples/canonical_exclusive/1_output.xml");

        let canonicalized = canonicalize_xml(
            input,
            CanonicalizationOptions {
                mode: CanonicalizationMode::ExclusiveCanonical1_0,
                keep_comments: true,
                inclusive_ns_prefixes: vec![],
            },
        )
        .unwrap();

        // for some reason, we get a stray \n at end of file :/
        assert_eq!(canonicalized, expected.trim())
    }

    #[test]
    fn canonical_exclusive_example_2() {
        let input = include_str!("samples/canonical_exclusive/2_input.xml");
        let expected = include_str!("samples/canonical_exclusive/2_output.xml");

        let canonicalized = canonicalize_xml(
            input,
            CanonicalizationOptions {
                mode: CanonicalizationMode::ExclusiveCanonical1_0,
                keep_comments: true,
                inclusive_ns_prefixes: ["stay1".to_string(), "stay2".to_string()].to_vec(),
            },
        )
        .unwrap();

        // for some reason, we get a stray \n at end of file :/
        assert_eq!(canonicalized, expected.trim())
    }

    #[test]
    fn invalid_xml() {
        let input = "<invalid xml";
        let canonicalized = canonicalize_xml(
            input,
            CanonicalizationOptions {
                mode: CanonicalizationMode::Canonical1_0,
                keep_comments: false,
                inclusive_ns_prefixes: vec![],
            },
        );
        assert!(canonicalized.is_err())
    }

    #[test]
    fn display_error() {
        let formatted = format!("{}", CanonicalizationErrorCode(-1));
        let expected = "canonicalization error (-1)";
        assert_eq!(formatted, expected);
    }
}
