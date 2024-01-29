use std::cell::Cell;
use std::mem::swap;
use std::error::Error;

#[non_exhaustive]
#[derive(Clone, Debug)]
enum Type {
    Literal(char),
    Dot,
    Star(Box<Type>),
    Plus(usize),
    Question(Box<Type>),
    Pipe,
    Begin,
    Match,
}

impl Type {
    fn captured_prev_state_is_valid(&self) -> bool {
        #[allow(clippy::match_like_matches_macro)]
        match self {
            Type::Star(s) | Type::Question(s) if matches!(s.as_ref(), Type::Star(_) | Type::Plus(_) | Type::Question(_)) => false,
            _ => true
        }
    }

    fn passed_prev_state_is_valid(&self, s: &Self) -> bool {
        #[allow(clippy::match_like_matches_macro)]
        match self {
            Type::Plus(_) if matches!(s, Type::Plus(_)) => false,
            Type::Pipe if matches!(s, Type::Pipe) => false,
            Type::Match if matches!(s, Type::Pipe) => false,
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
    anchors: Vec<usize>,
    ends: Vec<usize>,
    next_states: Vec<usize>,
}

impl Regex {

    fn new() -> Self {
        Self {
            states: vec![State::new(Type::Begin, vec![1]) ],
            ptr: 0,
            anchors: vec![0],
            ends: vec![],
            next_states: vec![0]
        }
    }

    // Update the previous nodes to include the passed pointer in their transitions
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

    // Update the anchor node at the top of the stack to include the passed
    // pointer in its transitions
    fn update_anchor_node(&mut self, ptr: usize) {
        let anchor_ptr = *self.anchors.last().unwrap();
        let anchor_state = &mut self.states[anchor_ptr];
        anchor_state.push_to_transitions(ptr);
    }

    // Update the nodes captured in the ends vector to include the passed
    // pointer in their transitions
    fn update_ends(&mut self, ptr: usize) {
        for end_ptr in &self.ends {
            let end_state = &mut self.states[*end_ptr];
            end_state.push_to_transitions(ptr);
        }
    }

    // Finish the regex by adding a `Match` state transition to the final node and all
    // of the end nodes captured in the ends vector. The `Match` state is the final state
    // in the state machine and is used to determine if the regex has matched the input.
    fn finish_regex(&mut self, last_state: Option<State>) -> Result<(), Box<dyn Error>> {
        let state = State::new(Type::Match, vec![]);

        if !state.t.passed_prev_state_is_valid(&last_state.unwrap().t) {
            return Err("Invalid regex, compilation failed".into())
        }
        let new_ptr = self.ptr + 1;
        self.update_previous_nodes(self.ptr, new_ptr);
        self.update_ends(new_ptr);
        self.states.push(state);
        self.ptr = 0;
        Ok(())
    }

    // Compile the regex pattern into a state machine. The state machine is as a vector of states
    // that hold references to the possible next states in the state machine.
    fn compile(pattern: &str) -> Result<Self, Box<dyn Error>> {
        let mut regex = Regex::new();
        let mut last_state = None;
        for c in pattern.chars() {
            match c {
                '.' => {
                    // create a new state that can always transition to the next state and push it to the state machine.
                    // The pointed is incremented here and the previous node's transitions are updated to include the new state
                    let state = State::new(Type::Dot, vec![]);
                    let new_ptr = regex.ptr + 1;
                    regex.update_previous_nodes(regex.ptr, new_ptr);
                    // Save the new state and head pointer
                    last_state = Some(state.clone());
                    regex.states.push(state);
                    regex.ptr = new_ptr;
                },
                '*' => {
                    // Take the current head of the state machine so it can be replaced
                    let t = regex.take_current_state().t;
                    // Create a new state to replace the current head with that can transition to itself.
                    let transitions = vec![regex.ptr];
                    let state = State::new(Type::Star(Box::new(t)), transitions);
                    // Check if the previous state is valid for a '*' meta character
                    if !state.t.captured_prev_state_is_valid() { return Err("Invalid regex, compilation failed".into()) }
                    // Save the new state and head pointer
                    last_state = Some(state.clone());
                    // Replace the previous head with the new state. The head pointer is not incremented here because we are
                    // replacing a state that was already in the state machine. We dont need to
                    // update the previous node either because the insertion of the original state
                    // added the pointer to the previous node's transitions
                    regex.states.push(state);
                },
                '+' => {
                    // Create a new state that can transition to itself or the next state. Here we
                    // increment the head pointer because we are adding a new state to the state machine
                    let new_ptr = regex.ptr + 1;
                    let transitions = vec![new_ptr];
                    let state = State::new(Type::Plus(regex.ptr), transitions);
                    // Get a reference to the current head of the state machine so we can clone it and use it in the new state
                    let t = &regex.get_current_state().t;
                    // check if the previous state is valid for a '+' meta character
                    if !state.t.passed_prev_state_is_valid(t) { return Err("Invalid regex, compilation failed".into()) }
                    // Save the new state and head pointer
                    last_state = Some(state.clone());
                    // Update the previous node's transitions to include the new state and store
                    // the new state and head pointer
                    regex.update_previous_nodes(regex.ptr, new_ptr);
                    regex.states.push(state);
                    regex.ptr = new_ptr;
                },
                '?' => {
                    // Take the current head of the state machine so it can be replaced
                    let t = regex.take_current_state().t;
                    // Create a new state that can transition to the next state or be skipped and push it to the state machine.
                    let state = State::new(Type::Question(Box::new(t)), vec![]);
                    // Check if the previous state is valid for a '?' meta character
                    if !state.t.captured_prev_state_is_valid() { return Err("Invalid regex, compilation failed".into()) }
                    // Save the new state and head pointer
                    last_state = Some(state.clone());
                    regex.states.push(state);
                },
                '|' => {
                    // Push the current head pointer to the ends vector. This will be used later to
                    // update the transitions of each end state to include the next state after the
                    // current groups ends. For the top level group this will be the `Match` state.
                    regex.ends.push(regex.ptr);
                    // Create a new state that is linked current anchor node and push it to the state machine.
                    let state = State::new(Type::Pipe, vec![]);
                    // Check if the previous state is valid for a '|' meta character
                    if !state.t.passed_prev_state_is_valid(&regex.get_current_state().t) { return Err("Invalid regex, compilation failed".into()) }
                    // Save the new state and head pointer
                    last_state = Some(state);
                    let new_ptr = regex.ptr + 1;
                    regex.update_anchor_node(new_ptr);
                },
                c => {
                    // Create a new state that can only transition to the next state if the contained literal matches
                    // and push it to the state machine.
                    let state = State::new(Type::Literal(c), vec![]);
                    let new_ptr = regex.ptr + 1;
                    regex.update_previous_nodes(regex.ptr, new_ptr);
                    // Save the new state and head pointer
                    last_state = Some(state.clone());
                    regex.states.push(state);
                    regex.ptr = new_ptr;
                }
            }
        }
        // Once we have finished processing the pattern we need to update the nodes in the end
        regex.finish_regex(last_state)?;
        Ok(regex)
    }

    // Take the current state from the head of the state machine
    // and return it
    fn take_current_state(&mut self) -> State {
        self.states.pop().unwrap()
    }

    // Get a reference to the current state at the head of the state machine
    fn get_current_state(&self) -> &State {
        &self.states[self.ptr]
    }

    // Get the next state from the next_states vector
    fn get_next_state(&mut self) -> Option<usize> {
        self.next_states.pop()
    }

    // Given a transition and a character, determine if the transition is valid.
    // If the transition is valid, push it to the next_states vector
    fn step(&self, transition: usize, c: char, new_states: &mut Vec<usize>) {
        match &self.states[transition].t {
            Type::Literal(ch) => {
                if c == *ch {
                    new_states.push(transition);
                    println!("character: {} matched state: {:?}", c, &self.states[transition].t);
                }
            },
            Type::Dot => {
                new_states.push(transition);
                println!("character: {} matched state: {:?}", c, &self.states[transition].t);
            },
            Type::Star(typ) | Type::Question(typ) => {
                match **typ {
                    Type::Literal(ch) => {
                        if c == ch {
                            new_states.push(transition);
                            println!("character: {} matched state: {:?}", c, &self.states[transition].t);
                        }
                    },
                    Type::Dot => {
                        new_states.push(transition);
                        println!( "{} matched state: {:?}", c, &self.states[transition].t);
                    },
                    _ => unreachable!(),
                };
            },
            Type::Plus(ptr) => {
                match self.states[*ptr].t {
                    Type::Literal(ch) => {
                        if c == ch {
                            new_states.push(transition);
                            println!("character: {} matched state: {:?}", c, &self.states[transition].t);
                        }
                    },
                    Type::Dot => {
                        new_states.push(transition);
                        println!( "{} matched state: {:?}", c, &self.states[transition].t);
                    },
                    _ => unreachable!(),
                };
            },
            Type::Pipe => {
                new_states.push(transition);
                println!( "{} matched state: {:?}", c, &self.states[transition].t);
            },
            Type::Match => {
                new_states.push(transition);
                println!( "{} matched state: {:?}", c, &self.states[transition].t);
            }
            Type::Begin => {
                new_states.push(transition);
                println!( "{} matched state: {:?}", c, &self.states[transition].t);
            }
        }
    }

    // Check if the state machine has found a match by
    // checking for the existence of a `Match` state in the next_states vector
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

    // Match the regex against the input string
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
    println!("checking: {} against: {}", s, p);
    let mut regex = Regex::compile(&p)?;
    println!("Regex: {:?}", regex);
    let haystack = HayStack::new(&s);
    Ok(regex.r#match(haystack))
}



fn main() -> Result<(), Box<dyn Error>> {
    // match is_match("aaaa".to_string(), "a*b?".to_string()) {
    //     Ok(true) => println!("Matched"),
    //     Ok(false) => println!("Not matched"),
    //     Err(e) => println!("Error: {}", e)
    // }
    match is_match("aaaa b  d".to_string(), "a+ b? c* d".to_string()) {
        Ok(true) => println!("Matched"),
        Ok(false) => println!("Not matched"),
        Err(e) => println!("Error: {}", e)
    }
    // match is_match("aaab".to_string(), "a*b?".to_string()) {
    //     Ok(true) => println!("Matched"),
    //     Ok(false) => println!("Not matched"),
    //     Err(e) => println!("Error: {}", e)
    // }
    // match is_match("aa".to_string(), "aa|bb".to_string()) {
    //     Ok(true) => println!("Matched"),
    //     Ok(false) => println!("Not matched"),
    //     Err(e) => println!("Error: {}", e)
    // }
    // match is_match("bb".to_string(), "aa|bb".to_string()) {
    //     Ok(true) => println!("Matched"),
    //     Ok(false) => println!("Not matched"),
    //     Err(e) => println!("Error: {}", e)
    // }
    // match is_match("cc".to_string(), "aa|bb|cc".to_string()) {
    //     Ok(true) => println!("Matched"),
    //     Ok(false) => println!("Not matched"),
    //     Err(e) => println!("Error: {}", e)
    // }
    Ok(())
}
