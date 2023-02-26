/// Provides useful hooks and supporting utilities.
pub mod hooks;
/// Holds platform-specific rendering interfaces.
pub mod renderer;
/// A reference-counted interior-mutable type designed to reduce runtime borrow rule violations.
pub mod shared;
/// Testing avalanche rendering, tracking, and hooks.
#[cfg(test)]
mod tests;
/// Holds types and macros used for propogating and tracking data updates.
pub mod tracked;
/// An in-memory representation of the current component tree.
#[doc(hidden)]
pub mod vdom;

use hooks::{HookContext, RenderContext};

use renderer::{DispatchNativeEvent, NativeEvent, NativeHandle, NativeType, Renderer};
use shared::Shared;
use tracked::Gen;
use vdom::{ComponentId, VDom};

pub use hooks::{state, store};
pub use tracked::Tracked;

/// An attribute macro used to define components.
///
/// # Basic usage
/// The macro must be applied to a function that returns a [View](View).
/// It generates a `struct` implementing [Component](Component) with the name of the input function, and a builder struct.
/// The function's name must start with a capital ASCII character.
///
/// The function can optionally take parameters. Parameter types must implement `Clone`.
/// Parameters must have concrete types: they cannot use the `impl Trait`
/// syntax. Components currently cannot be `async` or generic over types.
///
/// A component must return a [View] describing what it will render.
/// Components are invoked with the same syntax as function calls, except with
/// the macro `!` after the type name:
/// ```rust
/// use avalanche::{component, tracked, View};
/// use avalanche_web::components::{H1, Text};
///
/// const class: &str = "hello-world";
///
/// #[component]
/// pub fn HelloWorld(name: &str) -> View {
///     H1(
///         self,
///         id = class,
///         class = class,
///         child = Text(self, format!("Hi there, {}!", tracked!(name)))
///     )
/// }
/// ```
///
/// In the case of `H1`, all of its parameters are optional and have default values, but for components generated by this
/// macro, all parameters must be provided, or component instantiation will panic at runtime. In the future, this will
/// instead be a compile-time error.
///
/// Note that all macro invocations beginning with a capital ASCII character will be interpreted as component invocations
/// within the function. If you need to invoke a macro beginning with a capital letter, consider using it with an alias
/// beginning with `_`.
///
/// ## Tracked values
/// Parameter values are passed into a function with the [Tracked](Tracked) wrapper. More details on how that works is found in
/// the [tracked](tracked!) macro documentation.
///
/// ## Avoid side effects and mutability
/// `avalanche` considers a parameter updated if one of the parameters or hooks that influence it change, but
/// a function like [rand::thread_rng](https://docs.rs/rand/0.8/rand/fn.thread_rng.html) has a different value on every call
/// despite having no parameter or hook dependencies.
/// Using values from functions and methods that are not pure or mutate values will lead to missed updates. Instead, create new variables:
/// use `let string = string + " appended"` instead of `string += "appended"`.
/// If you need to use mutation in a component, including interior mutability, use the [state](state) hook.
///
/// ## Eschew third-party macros
/// Unfortunately, macros are syntax extensions and `component` cannot keep track of tracked variables in most of them.
/// All `std` macros (like [vec!](std::vec!()) and [format!](std::format!())) and `avalanche` macros (like [enclose!])
/// work well, but any others may lead to parameters being incorrectly marked as not updated.
#[doc(inline)]
pub use avalanche_macro::component;

/// Clones provided identifiers and passes them to the given expression.
/// The macro evaluates to that expression.
/// This is useful for passing data to multiple different sources that require `'static` data.
/// # Example
/// ```rust
/// # use avalanche::enclose;
/// let message = "Enclose me!".to_owned();
/// let second_message = String::new();
/// let closure1 = enclose!(message; move || println!("{message}"));
/// let closure2 = enclose!(message, second_message; move || println!("{message} again and {second_message}"));
/// ```
#[macro_export]
macro_rules! enclose {
    ( $( $x:ident ),*; $y:expr ) => {
        {
            $(let $x = $x.clone();)*
            $y
        }
    };
}

/// For internal use only, does not obey semver and is unsupported
/// A hack used to work around a seeming syn limitation
/// syn will interpret some macro calls as items rather than expressions
/// syn expects an item to be replaced by another, so when parsing Component! type macros,
/// `#[component]` will replace that call with an expression within this macro
/// to syn, this is replacing one macro item with another
#[doc(hidden)]
#[macro_export]
macro_rules! __internal_identity {
    ($e:expr) => {
        $e
    };
}

/// The return type of a component. Represents the child a component renders and returns.
pub struct View {
    /// The id of the component corresponding to the view, or None if it is ()
    id: Option<ComponentId>,
    /// The component id corresponding to the native component representation
    /// of the given tree, if it exists
    native_component_id: Option<ComponentId>,
}

impl View {
    fn private_copy(&self) -> Self {
        View {
            id: self.id,
            native_component_id: self.native_component_id,
        }
    }
}

impl From<()> for View {
    fn from((): ()) -> Self {
        Self {
            id: None,
            native_component_id: None,
        }
    }
}

impl From<Option<View>> for View {
    fn from(opt: Option<View>) -> Self {
        match opt {
            Some(val) => val,
            None => ().into(),
        }
    }
}

/// The trait representing a component.
///
/// Users should not implement this trait manually but instead use the `component` attribute.
/// However, native component implementations may need to use manual component implementations.
pub trait Component<'a>: Sized + 'a {
    fn render(self, render_ctx: RenderContext, hook_ctx: HookContext) -> View;

    fn updated(&self, gen: Gen) -> bool;

    fn native_type(&self) -> Option<NativeType> {
        None
    }

    #[allow(unused)]
    fn native_create(
        // TODO: should this be `self`?
        &self,
        renderer: &mut dyn Renderer,
        dispatch_native_event: DispatchNativeEvent,
    ) -> NativeHandle {
        panic!("Cannot call native_create on a non-native component")
    }

    #[allow(unused)]
    fn native_update(
        self,
        renderer: &mut dyn Renderer,
        native_type: &NativeType,
        native_handle: &mut NativeHandle,
        curr_gen: Gen,
        event: Option<NativeEvent>,
    ) -> Vec<View> {
        panic!("Cannot call native_update on a non-native component")
    }

    fn location(&self) -> Option<(u32, u32)> {
        None
    }

    fn key(&self) -> Option<String> {
        None
    }
}

impl<'a> Component<'a> for () {
    fn render(self, _: RenderContext, _: HookContext) -> View {
        View {
            id: None,
            native_component_id: None,
        }
    }
    fn updated(&self, _: Gen) -> bool {
        false
    }
}

/// A trait implemented for components that can be created without
/// any properties passed.
pub trait DefaultComponent<'a> {
    /// The type of the component implementation generated by the component
    /// builder.
    type Impl: Component<'a>;

    fn new() -> Self::Impl;
}

impl<'a> DefaultComponent<'a> for () {
    type Impl = ();

    fn new() {
        ()
    }
}

/// Internal data structure that stores what tree a component
/// belongs to, and its position within it
#[derive(Copy, Clone)]
pub(crate) struct ComponentPos<'a> {
    /// Shared value ONLY for passing to UseState
    /// within the render function this value is mutably borrowed,
    /// so exec and exec_mut will panic
    /// Shared container to the `VDom` of which the [`vnode`] is a part.
    pub(crate) vdom: &'a Shared<VDom>,
    /// Id of parent of component. Note that during a render it may
    /// not be present within the vdom.
    pub(crate) component_id: ComponentId,
}

#[derive(PartialEq, Eq, Hash)]
/// Represents a component macro invocation's unique identity within a component.
pub(crate) struct ChildId {
    pub location: (u32, u32),
    pub key: Option<String>,
}
