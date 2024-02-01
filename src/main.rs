use std::cell::Cell;
use std::mem::swap;
use std::error::Error;
use std::collections::VecDeque;

#[non_exhaustive]
#[derive(Clone, Debug)]
enum Type {
    Literal(u16),
    LiteralClass(Vec<u16>),
    Begin,
    Match,
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
    id: usize,
    t: Type,
    transitions: Vec<usize>
}

impl State {
    fn new(id: usize, r#type: Type, transitions: Vec<usize>) -> Self {
        State {
            id,
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
    anchors: VecDeque<usize>,
    starts: Vec<usize>,
    ends: Vec<usize>,
    next_states: Vec<usize>,
}

impl Regex {

    fn new() -> Self {
        Self {
            states: vec![State::new(0, Type::Begin, vec![1]) ],
            ptr: 0,
            anchors:  VecDeque::from_iter(vec![0usize]),
            starts: vec![0],
            ends: vec![],
            next_states: vec![0]
        }
    }

    // Update the previous nodes to include the passed pointer in their transitions
    fn update_previous_nodes(&mut self, ptr: usize, character: u16, last_char: char) {
        if character == '*' as u16 {
            let anchor = self.anchors.back().unwrap();
            let anchor_state = &mut self.states[*anchor];
            anchor_state.push_to_transitions(ptr);
        } else if character == '?' as u16 {
        } else if character == '+' as u16 {
            let anchor_state = &mut self.states[ptr];
            anchor_state.push_to_transitions(ptr);
            self.anchors = VecDeque::from_iter(vec![ptr]);
        } else {
            for anchor in &self.anchors  {
                let anchor_state = &mut self.states[*anchor];
                anchor_state.push_to_transitions(ptr);
            }
            self.anchors.push_back(ptr);
            if !['*', '?', '+', '|'].contains(&last_char) {
                self.anchors.pop_front();
            }
        }
    }

    // Update the anchor node at the top of the stack to include the passed
    // pointer in its transitions
    fn update_start_node(&mut self, ptr: usize) {
        let start_ptr = *self.starts.last().unwrap();
        let anchor_state = &mut self.states[start_ptr];
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
    fn finish_regex(&mut self, last_char: char) -> Result<(), Box<dyn Error>> {
        let new_ptr = self.ptr + 1;
        let state = State::new(new_ptr, Type::Match, vec![]);

        if last_char == '|' {
            return Err("Invalid regex, compilation failed".into())
        }
        self.update_previous_nodes(new_ptr, 257, last_char);
        self.update_ends(new_ptr);
        self.states.push(state);
        self.ptr = 0;
        Ok(())
    }

    // Compile the regex pattern into a state machine. The state machine is as a vector of states
    // that hold references to the possible next states in the state machine.
    fn compile(pattern: &str) -> Result<Self, Box<dyn Error>> {
        let mut regex = Regex::new();
        if pattern.is_empty() {
            return Err("Invalid regex, compilation failed. Empty patterns are not allowed".into())
        }
        let mut last_char = '\0';

        println!("Regex: {:?}", regex);
        for c in pattern.chars() {
            println!("Processing char: {}", c);
            match c {
                '.' => {
                    // create a new state that can always transition to the next state and push it to the state machine.
                    // The pointer is incremented here and the previous node's transitions are updated to include the new state
                    let new_ptr = regex.ptr + 1;
                    let state = State::new(new_ptr, Type::Literal(256), vec![]);
                    regex.update_previous_nodes(new_ptr, 256, last_char);
                    // Save the new state and head pointer
                    regex.states.push(state);
                    regex.ptr = new_ptr;
                },
                '*' => {
                    if last_char == '*' || last_char == '?' || last_char == '+' || last_char == '|' || last_char == '\0' {
                        return Err("Invalid regex, compilation failed".into())
                    }
                    regex.update_previous_nodes(regex.ptr, '*' as u16, last_char);
                },
                '+' => {
                    if last_char == '*' || last_char == '?' || last_char == '+' || last_char == '|' || last_char == '\0' {
                        return Err("Invalid regex, compilation failed".into())
                    }
                    regex.update_previous_nodes(regex.ptr, '+' as u16, last_char);
                },
                '?' => {
                    if last_char == '*' || last_char == '?' || last_char == '+' || last_char == '|' || last_char == '\0' {
                        return Err("Invalid regex, compilation failed".into())
                    }
                    regex.update_previous_nodes(regex.ptr, '?' as u16, last_char);
                },
                '|' => {
                    if last_char == '|' || last_char == '\0' {
                        return Err("Invalid regex, compilation failed".into())
                    }
                    let new_ptr = regex.ptr + 1;
                    regex.ends.push(regex.ptr);
                    regex.update_start_node(new_ptr);
                    regex.anchors = VecDeque::from_iter(vec![0]);
                },
                c => {
                    // Create a new state that can only transition to the next state if the contained literal matches
                    // and push it to the state machine.
                    let new_ptr = regex.ptr + 1;
                    let state = State::new(new_ptr, Type::Literal(c as u16), vec![]);
                    regex.update_previous_nodes(new_ptr, c as u16, last_char);
                    // Save the new state and head pointer
                    regex.states.push(state);
                    regex.ptr = new_ptr;
                }
            }
            last_char = c;
            println!("Regex: {:?}", regex);
        }
        // Once we have finished processing the pattern we need to update the nodes in the end
        regex.finish_regex(last_char)?;
        Ok(regex)
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
                if *ch == 256 || c as u16 == *ch {
                    new_states.push(transition);
                    println!("character: {} matched state: {:?}", c, &self.states[transition]);
                }
            },
            Type::LiteralClass(chars) => {
                if chars.contains(&(c as u16)) {
                    new_states.push(transition);
                    println!("character: {} matched state: {:?}", c, &self.states[transition]);
                }
            },
            Type::Match => {
                new_states.push(transition);
                println!( "{} matched state: {:?}", c, &self.states[transition]);
            }
            Type::Begin => {
                new_states.push(transition);
                println!( "{} matched state: {:?}", c, &self.states[transition]);
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
    match is_match("aaaa".to_string(), "a*b?".to_string()) {
        Ok(true) => println!("Matched"),
        Ok(false) => println!("Not matched"),
        Err(e) => println!("Error: {}", e)
    }
    match is_match("aaaaxd".to_string(), "a+b?xc*d".to_string()) {
        Ok(true) => println!("Matched"),
        Ok(false) => println!("Not matched"),
        Err(e) => println!("Error: {}", e)
    }
    match is_match("aaab".to_string(), "a*b?".to_string()) {
        Ok(true) => println!("Matched"),
        Ok(false) => println!("Not matched"),
        Err(e) => println!("Error: {}", e)
    }
    match is_match("aaac".to_string(), "a+b*c".to_string()) {
        Ok(true) => println!("Matched"),
        Ok(false) => println!("Not matched"),
        Err(e) => println!("Error: {}", e)
    }
    match is_match("aa".to_string(), "aa".to_string()) {
        Ok(true) => println!("Matched"),
        Ok(false) => println!("Not matched"),
        Err(e) => println!("Error: {}", e)
    }
    match is_match("c".to_string(), "a+|cb*".to_string()) {
        Ok(true) => println!("Matched"),
        Ok(false) => println!("Not matched"),
        Err(e) => println!("Error: {}", e)
    }

    Ok(())
}
