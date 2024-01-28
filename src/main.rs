use std::cell::Cell;
use std::mem::swap;
use std::error::Error;

#[non_exhaustive]
#[derive(Clone, Debug)]
enum Type {
    Literal(char),
    Dot,
    Star(Box<Type>),
    Plus(Box<Type>),
    Question(Box<Type>),
    Begin,
    Match,
}

impl Type {
    fn prev_state_is_valid(&self, s: Self) -> bool {
        #[allow(clippy::match_like_matches_macro)]
        match self {
            Type::Star(_) | Type::Plus(_) | Type::Question(_) if matches!(s, Type::Star(_) | Type::Plus(_) | Type::Question(_)) => false,
            _ => true
        }
    }
}

struct HayStack {
    s: Vec<char>,
    ptr: Cell<usize>,
}

impl HayStack {
    fn new(s: &str) -> Self {
        let s = s.chars().collect();
        HayStack {
            s,
            ptr: Cell::new(0),
        }
    }

    fn get_char(&mut self) -> Option<char> {
        let old_ptr = self.ptr.get();
        if old_ptr < self.s.len() {
            self.ptr.set(old_ptr + 1);
            Some(self.s[old_ptr])
        } else {
            None
        }
    }
}

#[derive(Clone, Debug)]
struct State {
    t: Type,
    transitions: Vec<usize>
}

impl State {
    fn new(r#type: Type, transitions: Vec<usize>) -> Self {
        State {
            t: r#type,
            transitions,
        }
    }

    fn push_to_transitions(&mut self, ptr: usize) {
        if !self.transitions.contains(&ptr) {
            self.transitions.push(ptr);
        }
    }
}

#[derive(Debug)]
struct Regex {
    states: Vec<State>,
    ptr: usize,
    next_states: Vec<usize>,
}

impl Regex {

    fn update_previous_nodes(&mut self, mut current_ptr: usize, ptr: usize) {
        if self.ptr == 0 {
            return
        }
        loop {
            let previous_state = &mut self.states[current_ptr];
            match previous_state.t {
                Type::Star(_) | Type::Plus(_) | Type::Question(_) => {
                    previous_state.push_to_transitions(ptr);
                },
                _ => {
                    previous_state.push_to_transitions(ptr);
                    break
                }
            }
            current_ptr -= 1;
        }
    }

    fn compile(pattern: &str) -> Result<Self, Box<dyn Error>> {
        let mut regex = Self { states: vec![State::new(Type::Begin, vec![1]) ], ptr: 0, next_states: vec![0] };
        let mut prev_state = None;
        for c in pattern.chars() {
            match c {
                '.' => {
                    // create a new state that can always transition to the next state and push it to the state machine.
                    // The pointed is incremented here and the previous node's transitions are updated to include the new state
                    let state = State::new(Type::Dot, vec![]);
                    let new_ptr = regex.ptr + 1;
                    regex.update_previous_nodes(regex.ptr, new_ptr);
                    regex.states.push(state);
                    regex.ptr = new_ptr;
                },
                '*' => {
                    // Take the current head of the state machine so it can be replaced
                    let t = regex.take_current_state().t;
                    // Check if the previous character is valid for a '*' meta character
                    if let Some(s) = prev_state {
                        if !t.prev_state_is_valid(s) { return Err("Invalid regex, compilation failed".into()) }
                    }
                    // Create a new state to replace the current head with that can transition to itself.
                    let transitions = vec![regex.ptr];
                    let state = State::new(Type::Star(Box::new(t)), transitions);
                    // Replace the previous head with the new state. The head pointer is not incremented here because we are
                    // replacing a state that was already in the state machine. We dont need to
                    // update the previous node either because the insertion of the original state
                    // added the pointer to the previous node's transitions
                    regex.states.push(state);
                },
                '+' => {
                    // Get a reference to the current head of the state machine so we can clone it and use it in the new state
                    let t = &regex.get_current_state().t;
                    // check if the previous character is valid for a '+' meta character
                    if let Some(s) = prev_state {
                        if !t.prev_state_is_valid(s) { return Err("Invalid regex, compilation failed".into()) }
                    }
                    // Create a new state that can transition to itself or the next state. Here we
                    // increment the head pointer because we are adding a new state to the state machine
                    let new_ptr = regex.ptr + 1;
                    let transitions = vec![new_ptr];
                    let state = State::new(Type::Plus(Box::new(t.clone())), transitions);

                    // Update the previous node's transitions to include the new state and store
                    // the new state and head pointer
                    regex.update_previous_nodes(regex.ptr, new_ptr);
                    regex.states.push(state);
                    regex.ptr = new_ptr;
                },
                '?' => {
                    // Take the current head of the state machine so it can be replaced
                    let t = regex.take_current_state().t;
                    // Check if the previous character is valid for a '?' meta character
                    if let Some(s) = prev_state {
                        if !t.prev_state_is_valid(s) { return Err("Invalid regex, compilation failed".into()) }
                    }
                    // Create a new state that can transition to the next state or be skipped and push it to the state machine.
                    let state = State::new(Type::Question(Box::new(t)), vec![]);
                    regex.states.push(state);
                }
                c => {
                    // Create a new state that can only transition to the next state if the contained literal matches
                    // and push it to the state machine.
                    let state = State::new(Type::Literal(c), vec![]);
                    let new_ptr = regex.ptr + 1;
                    regex.update_previous_nodes(regex.ptr, new_ptr);
                    regex.states.push(state);
                    regex.ptr = new_ptr;
                }
            }
            // Store the previous character so we can check if it is valid for a meta character
            prev_state = Some(regex.get_current_state().t.clone());
        }
        // Once we reached the end of the regex we need to add the `Match` state to the state
        // machine. The `Match` state has no transitions and indicates a match. After we add
        // the `Match` state we need to run the update_previous_nodes method to include the
        // `Match` state in the previous node and any other nodes that can transition to it via
        // a meta character. For example if the regex is "a*b?" this ensures that both "aaab" "aaa" will
        // match. This is done by adding the `Match` state to the transitions the of `Star` state and the final
        // `Question` state.
        let state = State::new(Type::Match, vec![]);
        let new_ptr = regex.ptr + 1;
        regex.update_previous_nodes(regex.ptr, new_ptr);
        regex.states.push(state);
        regex.ptr = 0;
        Ok(regex)
    }

    fn take_current_state(&mut self) -> State {
        self.states.pop().unwrap()
    }

    fn get_current_state(&self) -> &State {
        &self.states[self.ptr]
    }

    fn get_next_state(&mut self) -> Option<usize> {
        self.next_states.pop()
    }

    fn step(&self, transition: usize, c: char, new_states: &mut Vec<usize>) {
        match &self.states[transition].t {
            Type::Literal(ch) => {
                if c == *ch {
                    // println!("character: {} matched state: {:?}", c, &self.states[transition].t);
                    new_states.push(transition);
                }
            },
            Type::Dot => {
                // println!("character: {} matched state: {:?}", c, &self.states[transition].t);
                new_states.push(transition)
            },
            Type::Star(typ) | Type::Plus(typ) | Type::Question(typ) => {
                match **typ {
                    Type::Literal(ch) => {
                        if c == ch {
                            new_states.push(transition);
                            // println!("character: {} matched state: {:?}", c, &self.states[transition].t);
                        }
                    },
                    Type::Dot => {
                        new_states.push(transition);
                        // println!( "{} matched state: {:?}", c, &self.states[transition].t);
                    },
                    _ => unreachable!(),
                };
            },
            Type::Match => {
                new_states.push(transition);
            }
            Type::Begin => {
                new_states.push(transition);
            }
        }
    }

    fn found_match(&self) -> bool {
        for state in &self.next_states {
            for transition in &self.states[*state].transitions {
                // println!( "State {:?} at ptr {} has transition {:?} at ptr {}", &self.states[*state].t, state, &self.states[*transition].t, transition);
                if let Type::Match =  self.states[*transition].t {
                   return true
                }
            }
        }

        false
    }

    fn r#match(&mut self, mut haystack: HayStack) -> bool {
        while let Some(c) = haystack.get_char() {
            // println!("Processing char {}", c);
            let mut new_states = vec![];
            while let Some(ref current_state) = self.get_next_state() {
                let state = &self.states[*current_state];
                // println!("Processing state {:?}", &state);
                for transition in &state.transitions {
                    self.step(*transition, c, &mut new_states);
                };
            }
            swap(&mut self.next_states, &mut new_states);
        }
        self.found_match()
    }

}

pub fn is_match(s: String, p: String) -> Result<bool, Box<dyn Error>> {
    let mut regex = Regex::compile(&p)?;
    let haystack = HayStack::new(&s);
    Ok(regex.r#match(haystack))
}



fn main() -> Result<(), Box<dyn Error>> {
    match is_match("aaaa".to_string(), "a*b?".to_string()) {
        Ok(true) => println!("Matched"),
        Ok(false) => println!("Not matched"),
        Err(e) => println!("Error: {}", e)
    }
    match is_match("aaab".to_string(), "a*b?".to_string()) {
        Ok(true) => println!("Matched"),
        Ok(false) => println!("Not matched"),
        Err(e) => println!("Error: {}", e)
    }
    Ok(())
}
