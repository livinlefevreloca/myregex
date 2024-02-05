use std::cell::Cell;
use std::mem::swap;
use std::error::Error;
use std::collections::VecDeque;

fn main() {
    println!("Hello, world!");
}

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

#[allow(dead_code)] // id us not used but is convient for debugging
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
    group_anchors: Vec<usize>,
    starts: Vec<Vec<usize>>,
    ends: Vec<Vec<usize>>,
    next_states: Vec<usize>,
}

impl Regex {

    fn update_previous_nodes(&mut self, ptr: usize) {
        for anchor in &self.anchors {
            let anchor_state = &mut self.states[*anchor];
            anchor_state.push_to_transitions(ptr);
        }
    }

    fn handle_literal(&mut self, mut state: State, chars: &mut std::iter::Peekable<std::str::Chars>) -> bool {
        self.ptr += 1;
        let mut continue_loop = true;
        match chars.peek() {
            Some(&'*') => {
                chars.next();
                state.push_to_transitions(self.ptr);
                self.update_previous_nodes(self.ptr);
            },
            Some(&'?') => {
                chars.next();
                self.update_previous_nodes(self.ptr);
            },
            Some(&'+') => {
                chars.next();
                state.push_to_transitions(self.ptr);
                self.update_previous_nodes(self.ptr);
                self.anchors.clear();
            },
            Some(_) => {
                self.update_previous_nodes(self.ptr);
                self.anchors.clear();
            },
            None => {
                continue_loop = false;
                self.update_previous_nodes(self.ptr);
                self.anchors.clear();
            }
        }
        self.anchors.push_back(self.ptr);
        self.states.push(state);
        continue_loop
    }

    fn handle_close_group(&mut self, chars: &mut std::iter::Peekable<std::str::Chars>) -> bool {
        let start_ptr = self.group_anchors.pop().unwrap();
        let group_start_ptrs = self.starts.pop().unwrap();
        let group_end_ptrs = self.ends.pop().unwrap();
        let mut continue_loop = true;
        match chars.peek() {
            Some(&'*') => {
                chars.next();
                for group_start_ptr in group_start_ptrs {
                    for group_end_ptr in &group_end_ptrs {
                        self.states[*group_end_ptr].push_to_transitions(group_start_ptr);
                    }
                    self.states[start_ptr].push_to_transitions(group_start_ptr);
                    self.update_previous_nodes(group_start_ptr);
                }
                for group_end_ptr in &group_end_ptrs {
                    self.states[*group_end_ptr].push_to_transitions(self.ptr + 1);
                }

                self.states[start_ptr].push_to_transitions(self.ptr + 1);
            },
            Some(&'?') => {
                chars.next();
                for group_start_ptr in group_start_ptrs {
                    for group_end_ptr in &group_end_ptrs {
                        self.states[*group_end_ptr].push_to_transitions(group_start_ptr);
                    }
                    self.states[start_ptr].push_to_transitions(group_start_ptr);
                    self.update_previous_nodes(group_start_ptr);
                }
                for group_end_ptr in &group_end_ptrs {
                    self.states[*group_end_ptr].push_to_transitions(self.ptr + 1);
                }
                self.states[start_ptr].push_to_transitions(self.ptr + 1);
            },
            Some(&'+') => {
                chars.next();
                for group_start_ptr in group_start_ptrs {
                    for group_end_ptr in &group_end_ptrs {
                        self.states[*group_end_ptr].push_to_transitions(group_start_ptr);
                    }
                    self.states[start_ptr].push_to_transitions(group_start_ptr);
                    self.update_previous_nodes(group_start_ptr);
                }
                for group_end_ptr in &group_end_ptrs {
                    self.states[*group_end_ptr].push_to_transitions(self.ptr + 1);
                }

            },
            Some(_) => {
                for group_end_ptr in &group_end_ptrs {
                    self.states[*group_end_ptr].push_to_transitions(self.ptr + 1);
                }
            },
            None => {
                continue_loop = false;
            }
        }
        continue_loop
    }

    fn update_ends(&mut self, ptr: usize) {
        let ends = self.ends.pop().unwrap();
        for end in ends {
            self.states[end].push_to_transitions(ptr);
        }
    }

    fn finish_regex(&mut self) -> Result<(), Box<dyn Error>> {
        let new_ptr = self.ptr + 1;
        let state = State::new(new_ptr, Type::Match, vec![]);

        self.update_previous_nodes(new_ptr);
        self.update_ends(new_ptr);
        self.states.push(state);
        self.ptr = 0;
        Ok(())
    }

    fn compile(pattern: &str) -> Result<Self, Box<dyn Error>> {
        let mut regex = Self {
            states: vec![State::new(0, Type::Begin, vec![1]) ],
            ptr: 0,
            anchors:  VecDeque::from_iter(vec![0usize]),
            group_anchors: vec![0],
            starts: vec![vec![]],
            ends: vec![vec![]],
            next_states: vec![0]
        };

        if pattern.is_empty() {
            return Err("Invalid regex, compilation failed. Empty patterns are not allowed".into())
        }

        let mut chars = pattern.chars().peekable();
        eprintln!("Regex: {:?}", regex);
        while let Some(c) = chars.next() {
            eprintln!("Processing char: {}", c);
            match c {
                '.' => {
                    // If the next character is a dot we create a state with Literal containing 256
                    // to represent any character
                    let state = State::new(regex.ptr + 1, Type::Literal(256), vec![]);
                    if !regex.handle_literal(state, &mut chars) {
                        break
                    }
                },
                '|' => {
                    // A regex pattern cant end with a pipe
                    if chars.peek().is_none() {
                        return Err("Invalid regex, compilation failed. Invalid regex pattern".into())
                    }

                    // Add the current state to the next states to ends and starts respectively
                    for anchor in &regex.anchors {
                        regex.ends.last_mut().unwrap().push(*anchor);
                    }
                    regex.starts.last_mut().unwrap().push(regex.ptr + 1);

                    // Push the  next state to the transitions of the start of the current group
                    let start_ptr = *regex.group_anchors.last().unwrap();
                    regex.states[start_ptr].push_to_transitions(regex.ptr + 1);

                },
                '(' => {
                    regex.group_anchors.push(regex.ptr);
                    regex.starts.push(vec![regex.ptr + 1]);
                    regex.ends.push(vec![]);
                },
                ')' => {
                    regex.ends.last_mut().unwrap().push(regex.ptr);
                    eprintln!("ends: {:?}", regex.ends);
                    if !regex.handle_close_group(&mut chars) {
                        break
                    }
                },
                '*' | '+' | '?' => {
                    return Err("Invalid regex, compilation failed. Invalid regex pattern".into())
                },
                c => {
                    let state = State::new(regex.ptr + 1, Type::Literal(c as u16), vec![]);
                    if !regex.handle_literal(state, &mut chars) {
                        break
                    }
                }
            }
            eprintln!("Regex: {:?}", regex);
        }
        regex.finish_regex()?;
        eprintln!("\n\nRegex: {:?}\n\n", regex);
        Ok(regex)
    }

    fn get_next_state(&mut self) -> Option<usize> {
        self.next_states.pop()
    }

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

    fn found_match(&self) -> bool {
        for state in &self.next_states {
            for transition in &self.states[*state].transitions {
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
    println!("checking: {} against: {}", s, p);
    let mut regex = Regex::compile(&p)?;
    let haystack = HayStack::new(&s);
    Ok(regex.r#match(haystack))
}


#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test_is_match_same_char() {
        assert!(is_match("a".to_string(), "a".to_string()).unwrap());
    }

    #[test]
    fn test_no_match_different_char() {
        assert!(!is_match("a".to_string(), "b".to_string()).unwrap());
    }

    #[test]
    fn test_dot_match() {
        assert!(is_match("a".to_string(), ".".to_string()).unwrap());
    }

    #[test]
    fn test_dot_star_match() {
        assert!(is_match("This ShOuld maTch everYthing !@#$%^&*123456789".to_string(), ".*".to_string()).unwrap());
    }

    #[test]
    fn test_dot_plus_match() {
        assert!(is_match("This ShOuld maTch everYthing !@#$%^&*123456789".to_string(), ".+".to_string()).unwrap());
    }

    #[test]
    fn test_is_match_star() {
        assert!(is_match("aaa".to_string(), "a*".to_string()).unwrap());
    }

    #[test]
    fn test_is_match_star_no_exists_with_other_char() {
        assert!(is_match("b".to_string(), "a*b".to_string()).unwrap());
    }

    #[test]
    fn test_is_match_star_exists_with_other_char() {
        assert!(is_match("aab".to_string(), "a*b".to_string()).unwrap());
    }

    #[test]
    fn test_is_match_plus() {
        assert!(is_match("aaa".to_string(), "a+".to_string()).unwrap());
    }

    #[test]
    fn test_is_match_question_mark() {
        assert!(is_match("a".to_string(), "a?".to_string()).unwrap());
    }

    #[test]
    fn test_is_match_question_mark_exists_with_other_char() {
        assert!(is_match("ba".to_string(), "ba?".to_string()).unwrap());
    }

    #[test]
    fn test_is_match_question_mark_not_exists_with_other_char() {
        assert!(is_match("b".to_string(), "ba?".to_string()).unwrap());
    }

    #[test]
    fn test_is_match_parentheses() {
        assert!(is_match("a".to_string(), "(a)".to_string()).unwrap());
    }

    #[test]
    fn test_is_match_parentheses_plus() {
        assert!(is_match("abab".to_string(), "(ab)+".to_string()).unwrap());
    }

    #[test]
    fn test_is_match_parentheses_plus_with_other_char() {
        assert!(is_match("cababd".to_string(), "c(ab)+d".to_string()).unwrap());
    }

    #[test]
    fn test_is_match_parentheses_with_plus_in_paren() {
        assert!(is_match("cababbbd".to_string(), "c(ab+)+d".to_string()).unwrap());
    }
    #[test]
    fn test_is_not_match_parentheses_with_plus_in_paren() {
        assert!(!is_match("cabad".to_string(), "c(ab+)+d".to_string()).unwrap());
    }

    #[test]
    fn test_is_match_parentheses_with_star_in_paren() {
        assert!(is_match("caad".to_string(), "c(ab*)+d".to_string()).unwrap());
    }

    #[test]
    fn test_parentheses_start_with_star_in_paren() {
        assert!(is_match("aad".to_string(), "c?(ab*)+d".to_string()).unwrap());
    }

    #[test]
    fn test_parentheses_with_pipe() {
        assert!(is_match("cabd".to_string(), "c(ab|xy)d".to_string()).unwrap());
        assert!(is_match("cxyd".to_string(), "c(ab|xy)d".to_string()).unwrap());
    }
    #[test]
    fn test_parentheses_with_plus_with_pipe_first_option() {
        assert!(is_match("cababd".to_string(), "c(ab|xy)+d".to_string()).unwrap());
    }

    #[test]
    fn test_parentheses_with_plus_with_pipe_second_option() {
        assert!(is_match("cxyxyd".to_string(), "c(ab|xy)+d".to_string()).unwrap());
    }

    #[test]
    fn test_parentheses_plus_with_with_star_after() {
        assert!(is_match("cababa".to_string(), "c(ab)+d*a".to_string()).unwrap());
    }

    #[test]
    fn test_parentheses_plus_with_pipe_and_start_inside() {
        assert!(is_match("cazaza".to_string(), "c(az|bc*y)+d*a".to_string()).unwrap());
        assert!(is_match("cbcazda".to_string(), "c(az|bc*y)+d*a".to_string()).unwrap());
    }

    #[test]
    fn test_is_match_error_invalid_regexes() {
        assert!(is_match("_".to_string(), "*".to_string()).is_err());
        assert!(is_match("_".to_string(), "?".to_string()).is_err());
        assert!(is_match("_".to_string(), "+".to_string()).is_err());
        assert!(is_match("_".to_string(), "|".to_string()).is_err());
        assert!(is_match("_".to_string(), "a**".to_string()).is_err());
        assert!(is_match("_".to_string(), "a*+".to_string()).is_err());
        assert!(is_match("_".to_string(), "a*?".to_string()).is_err());
        assert!(is_match("_".to_string(), "a*|".to_string()).is_err());
        assert!(is_match("_".to_string(), "a?+".to_string()).is_err());
        assert!(is_match("_".to_string(), "a?*".to_string()).is_err());
        assert!(is_match("_".to_string(), "a??".to_string()).is_err());
        assert!(is_match("_".to_string(), "a?|".to_string()).is_err());
        assert!(is_match("_".to_string(), "a++".to_string()).is_err());
        assert!(is_match("_".to_string(), "a+*".to_string()).is_err());
        assert!(is_match("_".to_string(), "a+?".to_string()).is_err());
        assert!(is_match("_".to_string(), "a+|".to_string()).is_err());
    }

    #[test]
    fn test_is_match() {
        assert!(is_match("aaaaxd".to_string(), "a+b?xc*d".to_string()).unwrap());
        assert!(is_match("aaab".to_string(), "a*b?".to_string()).unwrap());
        assert!(is_match("aaab".to_string(), "a*b?b*".to_string()).unwrap());
        assert!(is_match("aaac".to_string(), "a+b*c".to_string()).unwrap());
        assert!(is_match("c".to_string(), "a+|cb*".to_string()).unwrap());
        assert!(is_match("aa".to_string(), "a+a|cb*".to_string()).unwrap());
        assert!(is_match("Babble Fish Test".to_string(), "Bab+le Fish .est".to_string()).unwrap());
    }

}
