//! Explicit port command lifecycle. Native Squad/GoalQueue execution remains separate.
use crate::{
    squad::{Order, State},
    Point,
};
impl State {
    pub(crate) fn invalidate_route(&mut self) {
        self.route.clear();
        self.waypoint = 0;
        self.next_plan = 0.;
    }
    pub(crate) fn issue_focus(&mut self, id: u32, point: Point, time: f64, tick: u64) {
        if !matches!(self.order, Order::Focus(_)) {
            self.return_order = (self.order == Order::Hold).then_some(Order::Hold);
        }
        self.order = Order::Focus(id);
        self.focus_point = Some(point);
        self.focus_until = time + 3.;
        self.invalidate_route();
        self.orders += 1;
        self.events.push((tick, format!("Focus{id}")));
    }
    pub(crate) fn toggle_order(&mut self, tick: u64) {
        self.order = if self.order == Order::Hold {
            Order::Follow
        } else {
            Order::Hold
        };
        self.return_order = None;
        self.focus_point = None;
        self.focus_until = 0.;
        self.invalidate_route();
        self.orders += 1;
        self.events.push((tick, format!("Order{:?}", self.order)));
    }
    pub(crate) fn finish_focus(&mut self, tick: u64, reason: &str) {
        if !matches!(self.order, Order::Focus(_)) {
            return;
        }
        self.order = self.return_order.take().unwrap_or(Order::Follow);
        self.focus_point = None;
        self.focus_until = 0.;
        self.invalidate_route();
        self.events.push((tick, format!("FocusFinished:{reason}")));
    }
    pub(crate) fn observe_focus(
        &mut self,
        target: Option<Point>,
        visible: bool,
        time: f64,
        tick: u64,
    ) {
        if !matches!(self.order, Order::Focus(_)) {
            return;
        }
        if target.is_none() {
            self.finish_focus(tick, "TargetGone");
        } else if visible {
            self.focus_point = target;
            self.focus_until = time + 3.;
        } else if time > self.focus_until {
            self.finish_focus(tick, "SightExpired");
        }
    }
    pub(crate) fn validate_orders(&self) -> Result<(), String> {
        if self
            .return_order
            .is_some_and(|o| o != Order::Hold || !matches!(self.order, Order::Focus(_)))
        {
            return Err("Invalid saved squad return order".into());
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn state() -> State {
        fn nav() -> crate::navigation::Assets {
            crate::navigation::Assets {
                provenance: serde_json::Value::Null,
                nodes: vec![],
                edges: vec![],
                lunge_distance: 0.,
                jump_distance: 0.,
                offensive_jump_probability: 0.,
            }
        }
        State::new(&crate::squad::Assets {
            defense: crate::melee::Defense {
                armor: 0.,
                include: vec![],
                exclude: vec![],
            },
            mesh: None,
            navigation: nav(),
            crouch_navigation: nav(),
            crouch_ratio: 0.5,
            weapon_snapshot: vec![],
            sounds: vec![],
            fire_sound: 0,
            health: 100,
            max_health: 400,
            shields: 100.,
            max_shields: 100.,
            shield_delay: 4.,
            shield_rate: 15.,
            ground_speed: 450.,
            damage: 20,
            shot_delay: 0.1,
            projectile_speed: 6000.,
            projectile_life: 10.,
            revive_duration: 2.7,
            provenance: serde_json::Value::Null,
        })
    }
    #[test]
    fn focus_replacement_and_target_death_resume_hold_once() {
        let mut s = state();
        s.toggle_order(1);
        s.issue_focus(42, [1., 2., 3.], 2., 2);
        s.issue_focus(43, [3., 4., 5.], 3., 3);
        s.route = vec![[100.; 3]];
        s.waypoint = 1;
        s.next_plan = 99.;
        s.observe_focus(None, false, 4., 4);
        assert_eq!(s.order, Order::Hold);
        assert!(s.return_order.is_none());
        assert!(s.route.is_empty());
        assert_eq!(s.waypoint, 0);
        assert_eq!(s.next_plan, 0.);
        assert!(s.focus_point.is_none());
        let count = s.events.len();
        s.observe_focus(None, false, 5., 5);
        assert_eq!(s.events.len(), count);
        assert_eq!(s.orders, 3);
    }
    #[test]
    fn hidden_target_keeps_last_seen_point_then_expires_to_follow() {
        let mut s = state();
        s.issue_focus(42, [1., 2., 3.], 1., 1);
        s.observe_focus(Some([99.; 3]), false, 4., 4);
        assert_eq!(s.focus_point, Some([1., 2., 3.]));
        assert_eq!(s.order, Order::Focus(42));
        s.observe_focus(Some([99.; 3]), false, 4.01, 5);
        assert_eq!(s.order, Order::Follow);
        assert!(s.focus_point.is_none());
    }
    #[test]
    fn visible_target_extends_deadline_and_explicit_order_cancels_focus() {
        let mut s = state();
        s.issue_focus(42, [1.; 3], 1., 1);
        s.observe_focus(Some([2.; 3]), true, 3., 3);
        assert_eq!(s.focus_until, 6.);
        assert_eq!(s.focus_point, Some([2.; 3]));
        s.toggle_order(4);
        assert_eq!(s.order, Order::Hold);
        s.observe_focus(None, false, 9., 9);
        assert_eq!(s.order, Order::Hold);
        assert!(s.return_order.is_none());
    }
    #[test]
    fn saved_temporary_command_resumes_and_legacy_state_stays_byte_identical() {
        let mut s = state();
        let legacy = serde_json::to_vec(&s).unwrap();
        assert!(!String::from_utf8_lossy(&legacy).contains("return_order"));
        let copy: State = serde_json::from_slice(&legacy).unwrap();
        assert_eq!(serde_json::to_vec(&copy).unwrap(), legacy);
        s.toggle_order(1);
        s.issue_focus(42, [1.; 3], 2., 2);
        let mut copy: State = serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
        copy.validate_orders().unwrap();
        copy.observe_focus(None, false, 3., 3);
        assert_eq!(copy.order, Order::Hold);
        copy.return_order = Some(Order::Focus(1));
        assert!(copy.validate_orders().is_err());
        copy.return_order = Some(Order::Hold);
        assert!(copy.validate_orders().is_err());
    }
    #[test]
    fn incapacitation_discards_follow_detour_before_revive_planning() {
        let mut g = crate::tests::game(false);
        let mut s = state();
        s.joined = true;
        s.route = vec![[900.; 3]];
        s.waypoint = 1;
        s.next_plan = 60.;
        g.state.squad = Some(s);
        assert!(g.try_incapacitate_player());
        let s = g.state.squad.as_ref().unwrap();
        assert!(s.player_down);
        assert!(s.route.is_empty());
        assert_eq!(s.waypoint, 0);
        assert_eq!(s.next_plan, 0.);
        assert!(!s.player_requested_revive);
    }
    #[test]
    fn checkpoint_rejects_recursive_return_command_atomically() {
        let mut g = crate::tests::game(false);
        let mut s = state();
        s.issue_focus(42, [100.; 3], 1., 0);
        s.return_order = Some(Order::Focus(43));
        g.state.squad = Some(s);
        g.initial = g.state.clone();
        let saved = g.checkpoint("commands").unwrap();
        assert!(g
            .restore_checkpoint(&saved, "commands")
            .unwrap_err()
            .contains("return order"));
        assert_eq!(g.checkpoint("commands").unwrap(), saved);
    }
}
