//! `smashline` is an open source project aimed at making code modifications to Super Smash Bros.
//! Ultimate an easier experience.
//!
//! You can find some top-level documentation on how SSBU manages character code and scripts on this
//! page, and you can also find more nuanced information about how this project functions on the
//! various other crate documentation pages.
//!
//! # Scripting Mechanisms
//! Based on some experience in modding games released on the switch, it appears that Nintendo most
//! likely offers assistance in some form, whether it be middleware or direct assistance, in
//! getting games and tools running with [Lua](https://www.lua.org/) a scripting language that
//! is pretty easy to integrate with and decently fast as well.
//!
//! SSBU makes use of Lua scripts heavily for its battle object scripting code, with some slight
//! caveats:
//! - `fighter` and `weapon` scripts are converted from compiled Lua byte-code into C++ source code
//! via an in-house transpiler called `lua2cpp`.
//! - `item` animation command (referred to as either AnimCMD or ACMD) scripts are kept as compiled
//! Lua byte-code in the game's romfs data and their status scripts are transpiled the same as
//! `fighter` and `weapon` scripts.
//!
//! Using Lua scripts in this fashion likely was a choice for quick development and easy integration
//! with other tools. Since Lua is a drop-in utility, the Lua scripts themselves were likely
//! auto-generated via a visual hitbox, effect, and sound effect placement/timeline tool.
//! Unfortunately, modding tools are not that advanced yet so we must resolved to writing our
//! scripts directly in code.
//!
//! SSBU also uses Lua scripts for *some* of its menus, however that use case is not supported by
//! this project.
//!
//! ## Animation Commands (AnimCMD or ACMD) and Lua coroutines
//! Another great reason to use Lua for this kind of process is that Lua comes with built-in
//! coroutine support, which makes a lot of sense to use in the case of a move that will be playing
//! out over the course of multiple frames.
//!
//! Here is an example Lua ACMD script, with documented comments about where the coroutines
//! will yield and how exactly they unyield:
//! ```lua
//! -- Start of the script
//!
//! -- Yield this coroutine, setting the unyield condition to once the frame timer is >= 5
//! frame(5)
//!
//! -- is_excute will be documented later
//! if is_excute() then
//!     WorkModule.on_flag(EXAMPLE_FLAG)
//! end
//!
//! -- Yield this coroutine, setting the unyield condition to once the frame
//! -- timer is >= (current_frame + 10)
//! wait(10)
//! if is_excute() then
//!     AttackModule.clear_all()
//! end
//!
//! -- Yield this coroutine, setting the unyield condition to once the frame timer is >= 30
//! frame(30)
//! if is_excute() then
//!     WorkModule.off_flag(EXAMPLE_FLAG)
//! end
//! ```
//!
//! This makes performing operations on specific frames a lot easier to both read and write,
//! since they are expressed as a set of procedural instructions instead of logic that might
//! be operating once on every frame.
//!
//! ### AnimCMD Categories
//! To further split up the logic into more maintainable components, there are four different
//! categories of ACMD scripts:
//! - `game` scripts
//!   
//!   `game` scripts operate on the core state of the object. Here you will find hitbox placements,
//! state manipulation, occasional button checks, etc.
//!
//!   An integral part of the animation is also exclusively controlled in these scripts: frame
//! pacing. Via calls to `FT_MOTION_RATE`, these scripts can change the speed of the animation and
//! how fast the script playback is. For this reason, calls to effect and sound libraries are
//! off-handed to the other scripts as a nice separation. The speed of those scripts are also
//! impacted by these calls.
//! - `effect` scripts
//!   
//!   `effect` scripts are purely visual, although changing them may lead to changes in how the
//! RNG generators work since some effects generate RNG values. Sword trails, singular effects,
//! flashes, screen backgrounds, etc. are usually all controlled by effects. They are not always
//! in the `effect` scripts, but for animations' associated effects that is where you will find
//! them.
//! - `sound` scripts
//!    
//!   `sound` scripts make calls to the game's sound module and libraries to either play sound
//! effects or pick a sound effect to play from a sound effect bank.
//! - `expression` scripts
//!
//!   `expression` scripts are basically the catch-all for everything else. Changing how an object
//! might display an interaction with sloped ground or how rumble is applied will happen in these
//! scripts.
//!
#![feature(new_uninit)]
#![allow(non_snake_case)]

pub mod api;
mod callbacks;
mod create_agent;
mod nro_hook;
mod state_callback;
mod static_accessor;
mod unwind;

#[skyline::main(name = "smashline-plugin")]
pub fn main() {
    create_agent::install_create_agent_hooks();
    create_agent::install_create_agent_share_hooks();
    create_agent::install_status_create_agent_hooks();
    nro_hook::install();
    state_callback::install_state_callback_hooks();
    callbacks::install_callback_hooks();
    unwind::install_unwind_patches();

    std::panic::set_hook(Box::new(|info| {
        let location = info.location().unwrap();

        let msg = match info.payload().downcast_ref::<&'static str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => &s[..],
                None => "Box<Any>",
            },
        };

        let err_msg = format!("smashline has panicked: '{}', {}", msg, location);
        skyline::error::show_error(
            69,
            "Smashline has panicked! Please PayPal $10 to the.blu.dev@gmail.com to fix it :)",
            err_msg.as_str(),
        );
    }));
}
