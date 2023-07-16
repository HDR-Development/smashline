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
