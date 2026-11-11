//! Aho-Corasick automaton of the mask patterns.
//!
//! Made here, without a dependency, for two reasons: (1) the automaton keeps the bytes of
//! the secrets, and a search library would keep copies that are not zeroed on drop (S8);
//! (2) the mask needs the state between chunks of the output and the depth of the state
//! (where the excerpt to cover starts). Nothing is held because of the state (D34
//! revised). It is not cryptography: it is text search, with time linear in the size of
//! the output even with repetitive patterns (`aaaa…` against `aaaa…`), so the output of
//! a command cannot stall the daemon.
//!
//! All the tables live in `Zeroizing<Vec<_>>` with the capacity computed before
//! inserting (no reallocation leaves a copy in freed memory) and are zeroed on drop.

use zeroize::Zeroizing;

/// "No node": the root (0) is never anyone's child nor sibling.
const NO_NODE: u32 = 0;
const ROOT: u32 = 0;

pub(super) struct Automaton {
    /// Byte of the edge that reaches each node.
    byte: Zeroizing<Vec<u8>>,
    /// First child (list linked by the siblings).
    child: Zeroizing<Vec<u32>>,
    /// Next sibling.
    sibling: Zeroizing<Vec<u32>>,
    /// Failure link.
    fail: Zeroizing<Vec<u32>>,
    /// Size of the longest pattern that ends in this state (0 = none).
    output: Zeroizing<Vec<u32>>,
    /// Depth of the node = size of the longest pattern prefix that matches now.
    depth: Zeroizing<Vec<u32>>,
    /// Children of the root in a direct table (the root is the most visited state).
    root: Zeroizing<Vec<u32>>,
}

/// Ensures room for `additional` more elements without leaving the old allocation in
/// plaintext: if it needs to grow, copies into a new one and the old one is zeroed when
/// dropped.
pub(super) fn reserve<T>(v: &mut Zeroizing<Vec<T>>, additional: usize)
where
    T: zeroize::Zeroize + Clone,
{
    let needed = v
        .len()
        .checked_add(additional)
        .expect("máscara: tamanho fora do limite");
    if needed <= v.capacity() {
        return;
    }
    let capacity = needed.max(v.capacity().saturating_mul(2)).max(16);
    let mut new = Zeroizing::new(Vec::with_capacity(capacity));
    new.extend_from_slice(v);
    *v = new;
}

fn push<T>(v: &mut Zeroizing<Vec<T>>, x: T)
where
    T: zeroize::Zeroize + Clone,
{
    reserve(v, 1);
    v.push(x);
}

fn index(n: u32) -> usize {
    n as usize
}

impl Automaton {
    /// Empty automaton with room for `nodes` nodes (including the root).
    pub(super) fn with_capacity(nodes: usize) -> Self {
        let nodes = nodes.max(1);
        let mut a = Automaton {
            byte: Zeroizing::new(Vec::with_capacity(nodes)),
            child: Zeroizing::new(Vec::with_capacity(nodes)),
            sibling: Zeroizing::new(Vec::with_capacity(nodes)),
            fail: Zeroizing::new(Vec::with_capacity(nodes)),
            output: Zeroizing::new(Vec::with_capacity(nodes)),
            depth: Zeroizing::new(Vec::with_capacity(nodes)),
            root: Zeroizing::new(vec![NO_NODE; 256]),
        };
        a.new_node(0, 0);
        a
    }

    fn new_node(&mut self, byte: u8, depth: u32) -> u32 {
        let id = u32::try_from(self.byte.len()).expect("máscara: nós demais");
        push(&mut self.byte, byte);
        push(&mut self.child, NO_NODE);
        push(&mut self.sibling, NO_NODE);
        push(&mut self.fail, ROOT);
        push(&mut self.output, 0);
        push(&mut self.depth, depth);
        id
    }

    /// Number of nodes (for the size tests).
    #[cfg(test)]
    pub(super) fn nodes(&self) -> usize {
        self.byte.len()
    }

    fn child_of(&self, node: u32, b: u8) -> Option<u32> {
        if node == ROOT {
            let c = self.root[usize::from(b)];
            return (c != NO_NODE).then_some(c);
        }
        let mut c = self.child[index(node)];
        while c != NO_NODE {
            if self.byte[index(c)] == b {
                return Some(c);
            }
            c = self.sibling[index(c)];
        }
        None
    }

    /// Adds a pattern. An empty pattern is ignored (it would match everything).
    pub(super) fn insert(&mut self, pattern: &[u8]) {
        if pattern.is_empty() {
            return;
        }
        let mut node = ROOT;
        for &b in pattern {
            node = match self.child_of(node, b) {
                Some(c) => c,
                None => {
                    let depth = self.depth[index(node)] + 1;
                    let new = self.new_node(b, depth);
                    if node == ROOT {
                        self.root[usize::from(b)] = new;
                    } else {
                        self.sibling[index(new)] = self.child[index(node)];
                        self.child[index(node)] = new;
                    }
                    new
                }
            };
        }
        let i = index(node);
        self.output[i] = self.depth[i];
    }

    /// Computes the failure links (breadth-first search) and propagates the outputs.
    pub(super) fn finalize(&mut self) {
        let mut queue: Zeroizing<Vec<u32>> = Zeroizing::new(Vec::with_capacity(self.byte.len()));
        for b in 0..=255u8 {
            let c = self.root[usize::from(b)];
            if c != NO_NODE {
                self.fail[index(c)] = ROOT;
                push(&mut queue, c);
            }
        }
        let mut head = 0;
        while head < queue.len() {
            let u = queue[head];
            head += 1;
            let mut v = self.child[index(u)];
            while v != NO_NODE {
                let b = self.byte[index(v)];
                let mut f = self.fail[index(u)];
                let target = loop {
                    if let Some(w) = self.child_of(f, b) {
                        break w;
                    }
                    if f == ROOT {
                        break ROOT;
                    }
                    f = self.fail[index(f)];
                };
                let iv = index(v);
                self.fail[iv] = target;
                let inherited = self.output[index(target)];
                if inherited > self.output[iv] {
                    self.output[iv] = inherited;
                }
                push(&mut queue, v);
                v = self.sibling[iv];
            }
        }
    }

    /// Next state after reading `b`.
    pub(super) fn step(&self, mut state: u32, b: u8) -> u32 {
        loop {
            if let Some(c) = self.child_of(state, b) {
                return c;
            }
            if state == ROOT {
                return ROOT;
            }
            state = self.fail[index(state)];
        }
    }

    /// Size of the longest pattern that ends in the state (0 = none).
    pub(super) fn output(&self, state: u32) -> usize {
        index(self.output[index(state)])
    }

    /// How many bytes of the end of the input can still be the start of a pattern.
    pub(super) fn depth(&self, state: u32) -> usize {
        index(self.depth[index(state)])
    }
}
