//! `field` subsystem — the field, ride and fade state the events poll.
//!
//! A headless port has no field to stream: the field is initialized, the player
//! object is loaded, the player stands on the ground, nothing is riding and no
//! fade is in progress. Those are exactly the answers the event scripts wait
//! for (`while component:IsLoading() do coroutine.yield() end`), so the
//! reference event can run past its load waits instead of parking in a bounded
//! 2 700-frame loop.
//!
//! Everything not modelled here falls back to recording stubs.

use mlua::{Lua, Result, Value};

use crate::host::{stub_fallback, SaveHandle};

fn answer(lua: &Lua, value: Value) -> Result<mlua::Function> {
    lua.create_function(move |_, _: mlua::MultiValue| Ok(value.clone()))
}

pub fn install(lua: &Lua, _save: &SaveHandle) -> Result<()> {
    // Character controller: the player stands, is not drowning.
    let controller = lua.create_table()?;
    controller.set("IsGrounded", answer(lua, Value::Boolean(true))?)?;
    controller.set("IsDrowning", answer(lua, Value::Boolean(false))?)?;
    controller.set("IsFalling", answer(lua, Value::Boolean(false))?)?;
    controller.set("IsMoving", answer(lua, Value::Boolean(false))?)?;
    stub_fallback(lua, &controller, "CharacterController")?;

    // Character component: exposes the controller.
    let charactor = lua.create_table()?;
    let controller_for_getter = controller.clone();
    charactor.set(
        "__haxe_export_GetCharacterController",
        lua.create_function(move |_, _: mlua::MultiValue| Ok(controller_for_getter.clone()))?,
    )?;
    stub_fallback(lua, &charactor, "CharactorComponent")?;

    // Field-object component: nothing is loading.
    let field_component = lua.create_table()?;
    field_component.set("IsLoading", answer(lua, Value::Boolean(false))?)?;
    stub_fallback(lua, &field_component, "FieldObjectComponent")?;

    // The player's field object.
    let field_object = lua.create_table()?;
    let charactor_for_getter = charactor.clone();
    field_object.set(
        "__haxe_export_FindCharactorComponent",
        lua.create_function(move |_, _: mlua::MultiValue| Ok(charactor_for_getter.clone()))?,
    )?;
    let component_for_getter = field_component.clone();
    field_object.set(
        "__haxe_export_FindFieldObjectComponent",
        lua.create_function(move |_, _: mlua::MultiValue| Ok(component_for_getter.clone()))?,
    )?;
    stub_fallback(lua, &field_object, "FieldObject")?;

    // Fade manager: every fade has finished.
    let fade = lua.create_table()?;
    fade.set("IsEnd", answer(lua, Value::Boolean(true))?)?;
    for name in ["RequestOut", "RequestIn", "Request", "ForceEnd"] {
        fade.set(name, lua.create_function(|_, _: mlua::MultiValue| Ok(()))?)?;
    }
    stub_fallback(lua, &fade, "FadeManager")?;

    // Ride manager: not riding, stopping is a no-op.
    let ride = lua.create_table()?;
    ride.set("IsRiding", answer(lua, Value::Boolean(false))?)?;
    ride.set("IsRideable", answer(lua, Value::Boolean(false))?)?;
    for name in ["StopRide", "StartRide", "Ride"] {
        ride.set(name, lua.create_function(|_, _: mlua::MultiValue| Ok(()))?)?;
    }
    stub_fallback(lua, &ride, "RideManager")?;

    // Field procedure: the field and the current area are initialized.
    let field_proc = lua.create_table()?;
    field_proc.set("IsEndInitializeField", answer(lua, Value::Boolean(true))?)?;
    field_proc.set(
        "IsEndInitializeCurrentArea",
        answer(lua, Value::Boolean(true))?,
    )?;
    stub_fallback(lua, &field_proc, "FieldProc")?;

    let global: mlua::Table = match lua.globals().get("Global") {
        Ok(t) => t,
        Err(_) => {
            let t = lua.create_table()?;
            lua.globals().set("Global", t.clone())?;
            t
        }
    };
    let get_field_object = {
        let field_object = field_object.clone();
        lua.create_function(move |_, _: mlua::MultiValue| Ok(field_object.clone()))?
    };
    global.set("GetFieldObjectForHaxe", get_field_object)?;
    let get_fade = {
        let fade = fade.clone();
        lua.create_function(move |_, _: mlua::MultiValue| Ok(fade.clone()))?
    };
    global.set("GetFadeManager", get_fade)?;
    let get_ride = {
        let ride = ride.clone();
        lua.create_function(move |_, _: mlua::MultiValue| Ok(ride.clone()))?
    };
    global.set("GetRideManager", get_ride)?;
    let find_field_proc = {
        let field_proc = field_proc.clone();
        lua.create_function(move |_, _: mlua::MultiValue| Ok(field_proc.clone()))?
    };
    global.set("FindFieldProc", find_field_proc)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_field_is_initialized_and_nothing_is_loading() {
        let lua = Lua::new();
        crate::host::install(&lua).unwrap();
        lua.load(
            r#"
            assert(Global.FindFieldProc():IsEndInitializeField() == true)
            assert(Global.FindFieldProc():IsEndInitializeCurrentArea() == true)
            local obj = Global.GetFieldObjectForHaxe()
            assert(obj:__haxe_export_FindFieldObjectComponent():IsLoading() == false)
            local ctrl = obj:__haxe_export_FindCharactorComponent():__haxe_export_GetCharacterController()
            assert(ctrl:IsGrounded() == true)
            assert(ctrl:IsDrowning() == false)
            assert(Global.GetFadeManager():IsEnd() == true)
            assert(Global.GetRideManager():IsRiding() == false)
            Global.GetRideManager():StopRide()
            "#,
        )
        .exec()
        .unwrap();
    }
}
