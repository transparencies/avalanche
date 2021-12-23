pub mod hooks;
/// A trait providing platform-specific rendering code.
pub mod renderer;
/// A reference-counted interior-mutable type designed to reduce runtime borrow rule violations.
pub mod shared;
pub mod tracked;
/// A `Vec`-backed implementation of a tree, with a relatively friendly mutation api.
pub(crate) mod tree;
/// An in-memory representation of the current component tree.
pub mod vdom;

use downcast_rs::{impl_downcast, Downcast};
use std::rc::Rc;

use renderer::NativeType;
use shared::Shared;
use tree::NodeId;
use vdom::{VDom, VNode};

pub use hooks::{state, vec, Context};
pub use tracked::Tracked;

/// An attribute macro used to define [Component](Component)s.
///
/// # Basic usage
/// The macro must be applied to a function that returns a [View](View).
/// It generates a `struct` implementing [Component](Component) with the name of the input function.
/// Thus, the function's name should start with a capital ASCII character to comply with
/// Rust type name conventions.
///
/// The function can optionally take parameters; users of the component will then be required to provide them,
/// as described below. Parameter types must be `'static`: that is, they cannot contain non-`'static` references.
/// The function body then receives each parameter as a reference, in order to avoid moving params
/// and allow components to be rendered multiple times. Parameters must have concrete types: they cannot use the `impl Trait`
/// syntax. Components cannot be `async` or generic.
///
/// A component must return a [View] describing what it will render.
/// A [View] contains an instance of a [Component]. The simplest possible [Component] to return is the `()` type:
/// ```rust
/// use avalanche::{component, View};
///
/// #[component]
/// pub fn Void() -> View {
///     ().into()
/// }
/// ```
/// The `()` type signifies that there is nothing to render. While this is sometimes useful, usually you will want
/// to render more complex components. Components are invoked with the same syntax as `struct` init expressions, except with
/// the macro `!` after the type name:
/// ```rust
/// use avalanche::{component, tracked, View};
/// use avalanche_web::components::{H1, Text};
///
/// const class: &str = "hello-world";
///
/// #[component]
/// pub fn HelloWorld(name: String) -> View {
///     H1!(
///         id: class,
///         class: class,
///         child: Text!(text: format!("Hi there, {}!", tracked!(name)))
///     )
/// }
/// ```
/// `id`, `class`, and `child` are all parameters of the `H1` component provided by the `avalanche_web` crate.
/// In order to create an instance of `H1`, we provide it its parameters as struct fields; this allows us to
/// provide the `class` parameter concisely instead of typing out `class: class`.
///
/// In the case of `H1`, all of its parameters are optional and have default values, but for components generated by this
/// macro, all parameters must be provided, or component instantiation will panic at runtime. In the future, this will
/// instead be a compile-time error, and specifying default values will be allowed.
///
/// Note that all macro invocations beginning with a capital ASCII character will be interpreted as component invocations
/// within the function. If you need to invoke a macro beginning with a capital letter, consider using it with an alias
/// beginning with `_`.
///
/// # Hooks
/// Pure components (those whose output only depends on their inputs) can be useful, but oftentimes you'll want
/// components with state to enable more complex behaviors. Hooks are composeable abstractions that enable you
/// to introduce state, side effects, and reusable functionality in your components.
///
/// Hooks are specified within the component attribute like this:
/// `#[component(name1 = HookType1, name2 = HookType2<u8>)]`
/// This injects two values, usually functions, with names `name1` and `name2` into your component's render code.
///
/// Unlike in some other frameworks, hook values can be used in any order and within any language construct,
/// although hooks often implement [`FnOnce`](std::ops::FnOnce) and thus can only be called once. A very commonly used
/// hook is [UseState<T>](UseState), as it allows injecting state into your component; check the linked docs for more details.
/// Custom hooks, defined by the `hook` attribute macro, will be introduced in a future version.
///
/// # Updates and dependency tracking
/// On each rerender, `avalanche` calculates whether each parameter of each component has been updated.
/// In order to enable this, the `component` attribute macro analyzes the dependency flow of parameters within UI code.
/// In each instance where a given `Component` is created with the syntax `Component !`, avalanche calculates which hooks
/// and parent parameter values contribute to the value of each child parameter. This enables efficienct updates,
/// but has a few caveats that creates two major rules to follow:
///
/// ## Avoid side effects and interior mutability
/// `avalanche` considers a parameter updated if one of the parameters or hooks that influence it change, but
/// a function like [rand::thread_rng](https://docs.rs/rand/0.8/rand/fn.thread_rng.html) has a different value on every call
/// despite having no parameter or hook dependencies.
/// Using values from functions and methods that are not pure or have interior mutability will lead to missed updates.
///
/// ## Eschew third-party macros
/// Unfortunately, macros have custom syntaxes and `component` cannot calculate the dependencies of most of them.
/// All `std` macros (like [vec!](std::vec!()) and [format!](std::format!())) and `avalanche` macros (like [enclose!()])
/// work well, but any others may lead to parameters being incorrectly marked as not updated.
#[doc(inline)]
pub use avalanche_macro::component;

/// Takes a list of identifiers terminated by a semicolon and expression. Each identifier is
/// cloned, and made available to the expression. The macro evaluates to that expression.
/// This is useful for passing things like state and setters to multiple component props.
/// # Example
/// ```rust
/// # use avalanche::enclose;
/// let message = "Enclose me!".to_owned();
/// let closure1 = enclose!(message; move || println!("{}", message));
/// let closure2 = enclose!(message; move || eprintln!("{}", message));
/// ```
// Note: code derived from stdweb
// TODO: appropriately license project and/or code snippet as MIT or Apache license
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

/// A reference-counted type that holds an instance of a component.
/// Component functions must return a [`View`].
#[derive(Clone)]
pub struct View {
    rc: Rc<dyn DynComponent>,
}

impl View {
    fn new<T: DynComponent>(val: T) -> Self {
        Self { rc: Rc::new(val) }
    }
}

impl std::ops::Deref for View {
    type Target = dyn DynComponent;

    fn deref(&self) -> &dyn DynComponent {
        &*self.rc
    }
}

impl<T: DynComponent> From<T> for View {
    fn from(val: T) -> Self {
        Self::new(val)
    }
}

impl<T: DynComponent> From<Option<T>> for View {
    fn from(val: Option<T>) -> Self {
        match val {
            Some(val) => View::new(val),
            None => View::new(()),
        }
    }
}

impl From<Option<View>> for View {
    fn from(val: Option<View>) -> Self {
        match val {
            Some(val) => val,
            None => View::new(()),
        }
    }
}

/// The trait representing a component. Except for renderer libraries,
/// users should not implement this trait manually but instead use the `component` attribute.
pub trait Component: 'static {
    type Builder;

    fn render(&self, ctx: Context) -> View;

    fn updated(&self) -> bool;

    fn native_type(&self) -> Option<NativeType> {
        None
    }

    fn location(&self) -> Option<(u32, u32)> {
        None
    }

    fn key(&self) -> Option<&str> {
        None
    }
}

impl Component for () {
    // TODO: make ! when never stabilizes
    type Builder = ();

    fn render(&self, _: Context) -> View {
        unreachable!()
    }
    fn updated(&self) -> bool {
        false
    }
}

/// An internal trait implemented for all [`Component`]s. This should not be
/// implemented manually.
#[doc(hidden)]
pub trait DynComponent: Downcast + 'static {
    fn render(&self, ctx: Context) -> View;

    fn native_type(&self) -> Option<NativeType>;

    fn updated(&self) -> bool;

    fn location(&self) -> Option<(u32, u32)>;

    fn key(&self) -> Option<&str>;
}

impl_downcast!(DynComponent);

impl<T: Component> DynComponent for T {
    fn render(&self, ctx: Context) -> View {
        Component::render(self, ctx)
    }

    fn native_type(&self) -> Option<NativeType> {
        Component::native_type(self)
    }

    fn updated(&self) -> bool {
        Component::updated(self)
    }

    fn location(&self) -> Option<(u32, u32)> {
        Component::location(self)
    }

    fn key(&self) -> Option<&str> {
        Component::key(self)
    }
}

#[doc(hidden)]
#[derive(Copy, Clone)]
pub struct ComponentNodeId {
    pub(crate) id: NodeId<VNode>,
}

impl From<NodeId<VNode>> for ComponentNodeId {
    fn from(node: NodeId<VNode>) -> Self {
        Self { id: node }
    }
}

#[doc(hidden)]
#[derive(Copy, Clone)]
/// Internal data structure that stores what tree a component
/// belongs to, and its position within it
pub struct ComponentPos<'a> {
    /// Shared value ONLY for passing to UseState
    /// within the render function this value is mutably borrowed,
    /// so exec and exec_mut will panic
    pub node_id: ComponentNodeId,
    /// Shared container to the VDom of which the [`vnode`] is a part.
    pub vdom: &'a Shared<VDom>,
}
