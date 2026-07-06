use bevy::prelude::*;

use crate::{InheritOutline, PropagateOutline, StopPropagateOutline};

#[derive(Clone, Component, Default)]
pub(crate) struct PropagateOutlineChild;

/// The components added to and removed from an entity that inherits an outline
/// via propagation.
type OutlineMarks = (InheritOutline, PropagateOutlineChild);

/// Adds the observers which propagate outlines to descendant entities.
pub(crate) fn add_propagate_observers(app: &mut App) {
    app.add_observer(on_add_propagate)
        .add_observer(on_remove_propagate)
        .add_observer(on_add_stop)
        .add_observer(on_remove_stop)
        .add_observer(on_insert_child_of)
        .add_observer(on_remove_child_of)
        .add_observer(on_add_child_marker)
        .add_observer(on_remove_child_marker);
}

fn mark_children(
    entity: Entity,
    children_q: &Query<&Children>,
    stop_q: Option<&Query<(), With<StopPropagateOutline>>>,
    source_q: &Query<(), With<PropagateOutline>>,
    commands: &mut Commands,
) {
    let Ok(children) = children_q.get(entity) else {
        return;
    };
    if stop_q.is_some_and(|q| q.contains(entity)) {
        return;
    }
    for child in children.iter() {
        if source_q.contains(child) {
            // A nested source manages its own subtree.
            continue;
        }
        commands.entity(child).try_insert(OutlineMarks::default());
    }
}

fn unmark_children(
    entity: Entity,
    children_q: &Query<&Children>,
    stop_q: Option<&Query<(), With<StopPropagateOutline>>>,
    source_q: &Query<(), With<PropagateOutline>>,
    commands: &mut Commands,
) {
    let Ok(children) = children_q.get(entity) else {
        return;
    };
    if stop_q.is_some_and(|q| q.contains(entity)) {
        return;
    }
    for child in children.iter() {
        if source_q.contains(child) {
            continue;
        }
        commands.entity(child).try_remove::<OutlineMarks>();
    }
}

fn propagates_to_children(
    entity: Entity,
    stop_q: &Query<(), With<StopPropagateOutline>>,
    source_q: &Query<(), With<PropagateOutline>>,
    marker_q: &Query<(), With<PropagateOutlineChild>>,
) -> bool {
    !stop_q.contains(entity) && (source_q.contains(entity) || marker_q.contains(entity))
}

fn on_add_child_marker(
    add: On<Add, PropagateOutlineChild>,
    children_q: Query<&Children>,
    stop_q: Query<(), With<StopPropagateOutline>>,
    source_q: Query<(), With<PropagateOutline>>,
    mut commands: Commands,
) {
    mark_children(
        add.entity,
        &children_q,
        Some(&stop_q),
        &source_q,
        &mut commands,
    );
}

fn on_remove_child_marker(
    remove: On<Remove, PropagateOutlineChild>,
    children_q: Query<&Children>,
    stop_q: Query<(), With<StopPropagateOutline>>,
    source_q: Query<(), With<PropagateOutline>>,
    mut commands: Commands,
) {
    unmark_children(
        remove.entity,
        &children_q,
        Some(&stop_q),
        &source_q,
        &mut commands,
    );
}

fn on_add_propagate(
    add: On<Add, PropagateOutline>,
    children_q: Query<&Children>,
    stop_q: Query<(), With<StopPropagateOutline>>,
    source_q: Query<(), With<PropagateOutline>>,
    mut commands: Commands,
) {
    let entity = add.entity;
    commands.entity(entity).try_remove::<OutlineMarks>();
    mark_children(entity, &children_q, Some(&stop_q), &source_q, &mut commands);
}

fn on_remove_propagate(
    remove: On<Remove, PropagateOutline>,
    children_q: Query<&Children>,
    parent_q: Query<&ChildOf>,
    stop_q: Query<(), With<StopPropagateOutline>>,
    source_q: Query<(), With<PropagateOutline>>,
    marker_q: Query<(), With<PropagateOutlineChild>>,
    mut commands: Commands,
) {
    let entity = remove.entity;
    if parent_q.get(entity).is_ok_and(|child_of| {
        propagates_to_children(child_of.parent(), &stop_q, &source_q, &marker_q)
    }) {
        // Rejoin the outer region as a marked child.
        commands.entity(entity).try_insert(OutlineMarks::default());
    } else {
        unmark_children(entity, &children_q, Some(&stop_q), &source_q, &mut commands);
    }
}

fn on_add_stop(
    add: On<Add, StopPropagateOutline>,
    children_q: Query<&Children>,
    source_q: Query<(), With<PropagateOutline>>,
    marker_q: Query<(), With<PropagateOutlineChild>>,
    mut commands: Commands,
) {
    let entity = add.entity;
    if source_q.contains(entity) || marker_q.contains(entity) {
        unmark_children(entity, &children_q, None, &source_q, &mut commands);
    }
}

fn on_remove_stop(
    remove: On<Remove, StopPropagateOutline>,
    children_q: Query<&Children>,
    source_q: Query<(), With<PropagateOutline>>,
    marker_q: Query<(), With<PropagateOutlineChild>>,
    mut commands: Commands,
) {
    let entity = remove.entity;
    if source_q.contains(entity) || marker_q.contains(entity) {
        mark_children(entity, &children_q, None, &source_q, &mut commands);
    }
}

fn on_insert_child_of(
    insert: On<Insert, ChildOf>,
    parent_q: Query<&ChildOf>,
    stop_q: Query<(), With<StopPropagateOutline>>,
    source_q: Query<(), With<PropagateOutline>>,
    marker_q: Query<(), With<PropagateOutlineChild>>,
    mut commands: Commands,
) {
    let entity = insert.entity;
    let Ok(child_of) = parent_q.get(entity) else {
        return;
    };
    if source_q.contains(entity) {
        return;
    }
    if propagates_to_children(child_of.parent(), &stop_q, &source_q, &marker_q) {
        commands.entity(entity).try_insert(OutlineMarks::default());
    } else if marker_q.contains(entity) {
        // Moved out of a propagating region.
        commands.entity(entity).try_remove::<OutlineMarks>();
    }
}

fn on_remove_child_of(
    remove: On<Remove, ChildOf>,
    marker_q: Query<(), With<PropagateOutlineChild>>,
    mut commands: Commands,
) {
    let entity = remove.entity;
    if marker_q.contains(entity) {
        commands.entity(entity).try_remove::<OutlineMarks>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds an app with the propagation observers registered.
    fn setup() -> App {
        let mut app = App::new();
        add_propagate_observers(&mut app);
        app
    }

    /// Spawns an entity, optionally as a child of `parent`, with `extra` bundle.
    fn spawn(app: &mut App, parent: Option<Entity>, extra: impl Bundle) -> Entity {
        let mut e = app.world_mut().spawn(extra);
        if let Some(parent) = parent {
            e.insert(ChildOf(parent));
        }
        e.id()
    }

    fn is_marked(app: &App, entity: Entity) -> bool {
        let world = app.world();
        world.get::<InheritOutline>(entity).is_some()
            && world.get::<PropagateOutlineChild>(entity).is_some()
    }

    fn is_unmarked(app: &App, entity: Entity) -> bool {
        let world = app.world();
        world.get::<InheritOutline>(entity).is_none()
            && world.get::<PropagateOutlineChild>(entity).is_none()
    }

    fn flush(app: &mut App) {
        app.world_mut().flush();
    }

    /// Builds `root -> a -> b -> c` with no propagation components.
    fn linear_chain(app: &mut App) -> (Entity, Entity, Entity, Entity) {
        let root = spawn(app, None, ());
        let a = spawn(app, Some(root), ());
        let b = spawn(app, Some(a), ());
        let c = spawn(app, Some(b), ());
        flush(app);
        (root, a, b, c)
    }

    #[test]
    fn test_add_propagate_marks_descendants() {
        let mut app = setup();
        let (root, a, b, c) = linear_chain(&mut app);

        app.world_mut().entity_mut(root).insert(PropagateOutline);
        flush(&mut app);

        assert!(
            is_unmarked(&app, root),
            "root (the source) must not be marked"
        );
        assert!(is_marked(&app, a));
        assert!(is_marked(&app, b));
        assert!(is_marked(&app, c));
    }

    #[test]
    fn test_remove_propagate_unmarks_descendants() {
        let mut app = setup();
        let (root, a, b, c) = linear_chain(&mut app);
        app.world_mut().entity_mut(root).insert(PropagateOutline);
        flush(&mut app);

        app.world_mut()
            .entity_mut(root)
            .remove::<PropagateOutline>();
        flush(&mut app);

        assert!(is_unmarked(&app, a));
        assert!(is_unmarked(&app, b));
        assert!(is_unmarked(&app, c));
    }

    #[test]
    fn test_add_stop_prunes_subtree() {
        let mut app = setup();
        let (root, a, b, c) = linear_chain(&mut app);
        app.world_mut().entity_mut(root).insert(PropagateOutline);
        flush(&mut app);

        // Stop at `b`.
        app.world_mut().entity_mut(b).insert(StopPropagateOutline);
        flush(&mut app);

        assert!(is_marked(&app, a));
        assert!(is_marked(&app, b), "the stop entity keeps its own mark");
        assert!(is_unmarked(&app, c));
    }

    #[test]
    fn test_remove_stop_restores_subtree() {
        let mut app = setup();
        let (root, a, b, c) = linear_chain(&mut app);
        app.world_mut().entity_mut(root).insert(PropagateOutline);
        app.world_mut().entity_mut(b).insert(StopPropagateOutline);
        flush(&mut app);
        assert!(is_unmarked(&app, c));

        app.world_mut()
            .entity_mut(b)
            .remove::<StopPropagateOutline>();
        flush(&mut app);

        assert!(is_marked(&app, a));
        assert!(is_marked(&app, b));
        assert!(is_marked(&app, c));
    }

    #[test]
    fn test_stop_present_before_propagate() {
        let mut app = setup();
        let root = spawn(&mut app, None, ());
        let a = spawn(&mut app, Some(root), StopPropagateOutline);
        let b = spawn(&mut app, Some(a), ());
        flush(&mut app);

        app.world_mut().entity_mut(root).insert(PropagateOutline);
        flush(&mut app);

        assert!(is_marked(&app, a), "the boundary itself is still marked");
        assert!(is_unmarked(&app, b), "the boundary blocks its children");
    }

    #[test]
    fn test_attach_new_child_to_marked_parent() {
        let mut app = setup();
        let (root, a, _b, _c) = linear_chain(&mut app);
        app.world_mut().entity_mut(root).insert(PropagateOutline);
        flush(&mut app);

        // Attach a new subtree under the marked `a`.
        let z = spawn(&mut app, Some(a), ());
        let z_child = spawn(&mut app, Some(z), ());
        flush(&mut app);

        assert!(is_marked(&app, z));
        assert!(is_marked(&app, z_child));
    }

    #[test]
    fn test_reparent_into_propagating_region() {
        let mut app = setup();
        let (root, a, _b, _c) = linear_chain(&mut app);
        app.world_mut().entity_mut(root).insert(PropagateOutline);

        // A detached subtree.
        let x = spawn(&mut app, None, ());
        let y = spawn(&mut app, Some(x), ());
        flush(&mut app);
        assert!(is_unmarked(&app, x));

        app.world_mut().entity_mut(x).insert(ChildOf(a));
        flush(&mut app);

        assert!(is_marked(&app, x));
        assert!(is_marked(&app, y));
    }

    #[test]
    fn test_reparent_out_of_region() {
        let mut app = setup();
        let (root, a, _b, _c) = linear_chain(&mut app);
        app.world_mut().entity_mut(root).insert(PropagateOutline);
        let x = spawn(&mut app, Some(a), ());
        let y = spawn(&mut app, Some(x), ());
        flush(&mut app);
        assert!(is_marked(&app, x));
        assert!(is_marked(&app, y));

        // Move `x` out to a non-propagating root.
        let other = spawn(&mut app, None, ());
        app.world_mut().entity_mut(x).insert(ChildOf(other));
        flush(&mut app);

        assert!(is_unmarked(&app, x));
        assert!(is_unmarked(&app, y));
    }

    #[test]
    fn test_orphan_removes_marks() {
        let mut app = setup();
        let (root, a, b, c) = linear_chain(&mut app);
        app.world_mut().entity_mut(root).insert(PropagateOutline);
        flush(&mut app);
        assert!(is_marked(&app, b));

        app.world_mut().entity_mut(b).remove::<ChildOf>();
        flush(&mut app);

        assert!(is_unmarked(&app, b));
        assert!(is_unmarked(&app, c));
        assert!(
            is_marked(&app, a),
            "ancestor above the orphan is unaffected"
        );
    }

    #[test]
    fn test_nested_propagate_boundary() {
        let mut app = setup();
        // root -> a -> d(source) -> e
        let root = spawn(&mut app, None, ());
        let a = spawn(&mut app, Some(root), ());
        let d = spawn(&mut app, Some(a), PropagateOutline);
        let e = spawn(&mut app, Some(d), ());
        flush(&mut app);
        assert!(is_marked(&app, e));
        assert!(is_unmarked(&app, d));

        app.world_mut().entity_mut(root).insert(PropagateOutline);
        flush(&mut app);
        assert!(is_marked(&app, a));
        assert!(
            is_unmarked(&app, d),
            "nested source is excluded from outer region"
        );
        assert!(is_marked(&app, e));

        // Remove the outer source; the nested region stays intact.
        app.world_mut()
            .entity_mut(root)
            .remove::<PropagateOutline>();
        flush(&mut app);
        assert!(is_unmarked(&app, a));
        assert!(is_unmarked(&app, d));
        assert!(is_marked(&app, e), "nested region is preserved");
    }

    #[test]
    fn test_nested_removal_rejoins_outer_region() {
        let mut app = setup();
        // root(source) -> a -> d(source) -> e
        let root = spawn(&mut app, None, PropagateOutline);
        let a = spawn(&mut app, Some(root), ());
        let d = spawn(&mut app, Some(a), PropagateOutline);
        let e = spawn(&mut app, Some(d), ());
        flush(&mut app);
        assert!(is_marked(&app, a));
        assert!(is_unmarked(&app, d));
        assert!(is_marked(&app, e));

        // Demote `d`; it rejoins the outer region.
        app.world_mut().entity_mut(d).remove::<PropagateOutline>();
        flush(&mut app);
        assert!(
            is_marked(&app, d),
            "demoted source rejoins the outer region"
        );
        assert!(is_marked(&app, e));
    }

    #[test]
    fn test_stop_takes_precedence_over_source() {
        let mut app = setup();
        // A source that is also stopped.
        let root = spawn(&mut app, None, (PropagateOutline, StopPropagateOutline));
        let a = spawn(&mut app, Some(root), ());
        let b = spawn(&mut app, Some(a), ());
        flush(&mut app);
        assert!(is_unmarked(&app, a), "stop takes precedence over source");
        assert!(is_unmarked(&app, b));

        // Lift the stop.
        app.world_mut()
            .entity_mut(root)
            .remove::<StopPropagateOutline>();
        flush(&mut app);
        assert!(is_marked(&app, a));
        assert!(is_marked(&app, b));

        // Re-apply the stop.
        app.world_mut()
            .entity_mut(root)
            .insert(StopPropagateOutline);
        flush(&mut app);
        assert!(is_unmarked(&app, a));
        assert!(is_unmarked(&app, b));
    }

    #[test]
    fn test_batch_spawn_cascade() {
        let mut app = setup();
        // Spawn the whole chain before flushing: each marker insert is still
        // queued when the next child attaches, yet the ripple must mark every
        // descendant.
        let root = spawn(&mut app, None, PropagateOutline);
        let a = spawn(&mut app, Some(root), ());
        let b = spawn(&mut app, Some(a), ());
        let c = spawn(&mut app, Some(b), ());
        flush(&mut app);

        assert!(is_unmarked(&app, root));
        assert!(is_marked(&app, a));
        assert!(is_marked(&app, b));
        assert!(is_marked(&app, c));
    }

    #[test]
    fn test_deep_batch_spawn() {
        let mut app = setup();
        // Eight levels below the source, all spawned before a single flush.
        let root = spawn(&mut app, None, PropagateOutline);
        let mut parent = root;
        let mut chain = Vec::new();
        for _ in 0..8 {
            parent = spawn(&mut app, Some(parent), ());
            chain.push(parent);
        }
        flush(&mut app);

        assert!(is_unmarked(&app, root));
        for entity in chain {
            assert!(is_marked(&app, entity), "every level must be marked");
        }
    }

    #[test]
    fn test_promote_marked_child_to_source() {
        let mut app = setup();
        // root(source) -> a -> b, with `a` and `b` as marked children.
        let root = spawn(&mut app, None, PropagateOutline);
        let a = spawn(&mut app, Some(root), ());
        let b = spawn(&mut app, Some(a), ());
        flush(&mut app);
        assert!(is_marked(&app, a));
        assert!(is_marked(&app, b));

        // Promote `a` to a source: it sheds its own mark but keeps `b` marked,
        // now inheriting from `a` instead of `root`.
        app.world_mut().entity_mut(a).insert(PropagateOutline);
        flush(&mut app);

        assert!(
            is_unmarked(&app, a),
            "a is now a source, not a marked child"
        );
        assert!(is_marked(&app, b), "b keeps inheriting from the new source");
    }

    #[test]
    fn test_batch_spawn_cascade_deferred() {
        let mut app = setup();
        // Build the whole chain via deferred `Commands`: nothing applies until
        // the single flush.
        let (root, a, b, c);
        {
            let world = app.world_mut();
            let mut commands = world.commands();
            root = commands.spawn(PropagateOutline).id();
            a = commands.spawn(ChildOf(root)).id();
            b = commands.spawn(ChildOf(a)).id();
            c = commands.spawn(ChildOf(b)).id();
        }
        app.world_mut().flush();

        assert!(is_unmarked(&app, root));
        assert!(is_marked(&app, a));
        assert!(is_marked(&app, b));
        assert!(is_marked(&app, c));
    }

    #[test]
    fn test_reparent_marked_subtree_under_pending_parent_deferred() {
        let mut app = setup();
        let root = spawn(&mut app, None, PropagateOutline);
        let x = spawn(&mut app, Some(root), ());
        let y = spawn(&mut app, Some(x), ());
        flush(&mut app);
        assert!(is_marked(&app, x));
        assert!(is_marked(&app, y));

        // Via deferred `Commands`: spawn `p` under the source and reparent `x`
        // under `p`, all pending until flush.
        {
            let world = app.world_mut();
            let mut commands = world.commands();
            let p = commands.spawn(ChildOf(root)).id();
            commands.entity(x).insert(ChildOf(p));
        }
        app.world_mut().flush();

        assert!(is_marked(&app, x));
        assert!(is_marked(&app, y));
    }

    #[test]
    fn test_reparent_marked_subtree_under_pending_parent() {
        let mut app = setup();
        let root = spawn(&mut app, None, PropagateOutline);
        flush(&mut app);

        // A marked subtree `x -> y` under the source.
        let x = spawn(&mut app, Some(root), ());
        let y = spawn(&mut app, Some(x), ());
        flush(&mut app);
        assert!(is_marked(&app, x));
        assert!(is_marked(&app, y));

        // One unflushed batch: attach `p` under the source (its mark still
        // queued) then reparent `x` under `p`; the whole `p -> x -> y` chain
        // must end marked.
        let p = spawn(&mut app, Some(root), ());
        app.world_mut().entity_mut(x).insert(ChildOf(p));
        flush(&mut app);

        assert!(is_marked(&app, p));
        assert!(is_marked(&app, x));
        assert!(is_marked(&app, y));
    }
}
