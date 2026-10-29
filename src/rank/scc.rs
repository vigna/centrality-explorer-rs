//! Strongly connected components.

/// Returns the strongly connected components of a graph given by successor
/// lists.
///
/// Each component is sorted, and components are returned in reverse
/// topological order: a component appears before every component that has
/// an arc into it, so components with no outgoing arcs come first.
///
/// # Implementation Notes
///
/// This is Tarjan's algorithm with an explicit stack, so that large graphs
/// cannot overflow the call stack.
pub fn sccs(succ: &[Vec<usize>]) -> Vec<Vec<usize>> {
    const UNVISITED: usize = usize::MAX;
    let n = succ.len();
    let mut index = vec![UNVISITED; n];
    let mut low = vec![0; n];
    let mut on_stack = vec![false; n];
    let mut stack = Vec::new();
    let mut components = Vec::new();
    let mut counter = 0;
    // Each frame is a node and the position of its next successor to visit.
    let mut frames: Vec<(usize, usize)> = Vec::new();

    for root in 0..n {
        if index[root] != UNVISITED {
            continue;
        }
        index[root] = counter;
        low[root] = counter;
        counter += 1;
        stack.push(root);
        on_stack[root] = true;
        frames.push((root, 0));

        while let Some(&(v, pos)) = frames.last() {
            if let Some(&w) = succ[v].get(pos) {
                frames.last_mut().expect("frames is not empty").1 += 1;
                if index[w] == UNVISITED {
                    index[w] = counter;
                    low[w] = counter;
                    counter += 1;
                    stack.push(w);
                    on_stack[w] = true;
                    frames.push((w, 0));
                } else if on_stack[w] {
                    low[v] = low[v].min(index[w]);
                }
            } else {
                frames.pop();
                if let Some(&(u, _)) = frames.last() {
                    low[u] = low[u].min(low[v]);
                }
                if low[v] == index[v] {
                    let mut component = Vec::new();
                    loop {
                        let w = stack.pop().expect("v is on the stack");
                        on_stack[w] = false;
                        component.push(w);
                        if w == v {
                            break;
                        }
                    }
                    component.sort_unstable();
                    components.push(component);
                }
            }
        }
    }
    components
}
