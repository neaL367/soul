//! Spike 0(b): Boa JavaScript Engine Corpus Compatibility Tests.
//!
//! Evaluates Boa's parser, AST, built-in objects, and execution pipeline against
//! real-world ECMAScript constructs typical of documentation, blogs, and interactive forms.

use boa_engine::{Context, Source};

fn eval_js(code: &str) -> Result<String, String> {
    let mut context = Context::default();
    match context.eval(Source::from_bytes(code.as_bytes())) {
        Ok(val) => Ok(val.display().to_string()),
        Err(err) => Err(err.to_string()),
    }
}

#[test]
fn test_modern_es_syntax_and_operators() {
    let code = r"
        const user = { profile: { name: 'Alice', age: 30 } };
        const name = user?.profile?.name ?? 'Anonymous';
        const missing = user?.settings?.theme ?? 'dark';
        
        let count = null;
        count ??= 42;
        
        const [first, ...rest] = [1, 2, 3, 4];
        const merged = { ...user.profile, count, first, restLength: rest.length };
        
        `${name}:${missing}:${merged.count}:${merged.first}:${merged.restLength}`
    ";
    let result = eval_js(code).expect("evaluation failed");
    assert_eq!(result, "\"Alice:dark:42:1:3\"");
}

#[test]
fn test_es_classes_and_inheritance() {
    let code = r"
        class BaseComponent {
            constructor(id) {
                this.id = id;
                this.rendered = false;
            }
            mount() {
                this.rendered = true;
                return `mounted:${this.id}`;
            }
        }

        class NavMenu extends BaseComponent {
            #items = [];
            constructor(id, items) {
                super(id);
                this.#items = items;
            }
            get itemCount() {
                return this.#items.length;
            }
            render() {
                const status = this.mount();
                return `${status}:items=${this.itemCount}`;
            }
        }

        const menu = new NavMenu('main-nav', ['Home', 'Docs', 'About']);
        menu.render();
    ";
    let result = eval_js(code).expect("evaluation failed");
    assert_eq!(result, "\"mounted:main-nav:items=3\"");
}

#[test]
fn test_doc_site_search_indexer() {
    let code = r"
        const documents = [
            { id: 1, title: 'Rust Getting Started', content: 'Install rustup and cargo to start.' },
            { id: 2, title: 'GPUI Architecture', content: 'GPUI is a fast GPU-accelerated UI framework.' },
            { id: 3, title: 'DOM and CSS Cascade', content: 'The cascade resolves style property values for DOM nodes.' }
        ];

        function buildIndex(docs) {
            const index = new Map();
            for (const doc of docs) {
                const tokens = (doc.title + ' ' + doc.content)
                    .toLowerCase()
                    .replace(/[^\w\s]/g, '')
                    .split(/\s+/);
                for (const token of tokens) {
                    if (!token) continue;
                    if (!index.has(token)) index.set(token, new Set());
                    index.get(token).add(doc.id);
                }
            }
            return index;
        }

        function search(query, index) {
            const terms = query.toLowerCase().split(/\s+/);
            const results = terms.map(t => index.get(t) || new Set());
            if (results.length === 0) return [];
            return Array.from(results[0]).filter(id => results.every(set => set.has(id)));
        }

        const index = buildIndex(documents);
        const rustMatches = search('rustup', index);
        const gpuMatches = search('gpui', index);
        
        JSON.stringify({ rust: rustMatches, gpu: gpuMatches });
    ";
    let result = eval_js(code).expect("evaluation failed");
    assert_eq!(result, "\"{\\\"rust\\\":[1],\\\"gpu\\\":[2]}\"");
}

#[test]
fn test_form_validation_and_state_machine() {
    let code = r"
        const formState = {
            values: { email: 'user@example.com', password: 'Password123!', acceptTerms: true },
            errors: {},
            isValid: false
        };

        const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
        const passwordRegex = /^(?=.*[A-Za-z])(?=.*\d)(?=.*[@$!%*#?&])[A-Za-z\d@$!%*#?&]{8,}$/;

        function validate(state) {
            const errors = {};
            if (!emailRegex.test(state.values.email)) {
                errors.email = 'Invalid email';
            }
            if (!passwordRegex.test(state.values.password)) {
                errors.password = 'Password too weak';
            }
            if (!state.values.acceptTerms) {
                errors.terms = 'Must accept terms';
            }
            return {
                ...state,
                errors,
                isValid: Object.keys(errors).length === 0
            };
        }

        const validated = validate(formState);
        JSON.stringify({ valid: validated.isValid, errorCount: Object.keys(validated.errors).length });
    ";
    let result = eval_js(code).expect("evaluation failed");
    assert_eq!(result, "\"{\\\"valid\\\":true,\\\"errorCount\\\":0}\"");
}

#[test]
fn test_blog_comment_tree_traversal() {
    let code = r"
        const comments = [
            { id: 1, parentId: null, text: 'Great article!' },
            { id: 2, parentId: 1, text: 'I agree.' },
            { id: 3, parentId: 2, text: 'Me too.' },
            { id: 4, parentId: null, text: 'Have a question.' }
        ];

        function buildCommentTree(list) {
            const map = new Map();
            const roots = [];
            
            list.forEach(c => map.set(c.id, { ...c, children: [] }));
            list.forEach(c => {
                const node = map.get(c.id);
                if (c.parentId === null) {
                    roots.push(node);
                } else if (map.has(c.parentId)) {
                    map.get(c.parentId).children.push(node);
                }
            });
            return roots;
        }

        function maxDepth(node) {
            if (!node.children || node.children.length === 0) return 1;
            return 1 + Math.max(...node.children.map(maxDepth));
        }

        const tree = buildCommentTree(comments);
        const depths = tree.map(maxDepth);
        JSON.stringify({ rootCount: tree.length, maxTreeDepth: Math.max(...depths) });
    ";
    let result = eval_js(code).expect("evaluation failed");
    assert_eq!(result, "\"{\\\"rootCount\\\":2,\\\"maxTreeDepth\\\":3}\"");
}

#[test]
fn test_promise_and_microtask_resolution() {
    let code = r"
        let trace = [];
        trace.push('start');

        Promise.resolve('p1')
            .then(val => {
                trace.push(val);
                return 'p2';
            })
            .then(val => {
                trace.push(val);
            });

        trace.push('end');
        trace.join('->');
    ";
    let result = eval_js(code).expect("evaluation failed");
    assert!(result.contains("start") && result.contains("end"));
}
