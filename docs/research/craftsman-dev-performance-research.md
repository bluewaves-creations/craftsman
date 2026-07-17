# Performance Testing as Fitness Functions: Research & Architecture

> Response time budgets, bundle size limits, and Core Web Vitals as mechanically verifiable performance gates — evaluated against the 2026 landscape of Lighthouse CI, k6, and performance budget tooling.

---

## The Problem

Functional correctness (Gherkin), structural quality (CodeHealth), visual fidelity (Playwright), accessibility (axe-core), architecture (fitness functions), and security (SAST/SCA) — but no performance gate. The agent can make every test pass while producing a 5MB JavaScript bundle, a 4-second LCP, or an API endpoint that takes 2 seconds under load. Performance degrades incrementally — one dependency, one image, one query at a time — and without a mechanical gate, the degradation is invisible until users feel it.

## Performance as Fitness Functions

Performance budgets are fitness functions. They define measurable thresholds that the codebase must stay within. When a threshold is crossed, the build fails. Same architecture as architectural fitness functions, same pass/fail contract.

Three categories of performance fitness functions:

### 1. Front-End: Lighthouse CI (Core Web Vitals)

Lighthouse CI runs Lighthouse on every commit and fails builds that don't meet thresholds. Free, open-source, integrates with GitHub Actions and all major CI platforms.

```javascript
// lighthouserc.js
module.exports = {
  ci: {
    assert: {
      assertions: {
        'categories:performance': ['error', { minScore: 0.8 }],
        'largest-contentful-paint': ['error', { maxNumericValue: 2500 }],
        'cumulative-layout-shift': ['error', { maxNumericValue: 0.1 }],
        'total-blocking-time': ['error', { maxNumericValue: 300 }],
        'resource-summary:script:size': ['error', { maxNumericValue: 500000 }],
      }
    }
  }
};
```

When a budget breaks, the error is precise: "LCP exceeded budget of 2500ms (actual: 3100ms)." The developer (or agent) fetches the Lighthouse artifact, identifies what changed, and fixes it.

### 2. API: k6 / Response Time Budgets

k6 (Grafana, open-source) runs load tests with response time thresholds. A 20-line script checks the most critical API endpoint.

```javascript
import http from 'k6/http';
import { check } from 'k6';

export const options = {
  thresholds: {
    http_req_duration: ['p95<500'], // 95th percentile under 500ms
  },
};

export default function () {
  const res = http.get('http://localhost:3000/api/todos');
  check(res, { 'status is 200': (r) => r.status === 200 });
}
```

### 3. Bundle Size: Size-Limit / Bundlewatch

Deterministic bundle size checking. The budget is a JSON file committed to the repo:

```json
[
  { "path": "dist/index.js", "limit": "50 kB" },
  { "path": "dist/vendor.js", "limit": "150 kB" }
]
```

If a dependency or code change pushes the bundle over budget, the build fails with the exact size delta.

## Integration with Craftsman Dev

Performance gates are context-dependent — not every project needs all three:

**Web front-end projects:** Lighthouse CI (CWV + bundle size) at batch boundaries for front-end batches.

**API/backend projects:** k6 response time budgets for critical endpoints at batch boundaries.

**Full-stack projects:** both, running in parallel.

```bash
craftsman perf          # Lighthouse CI + k6 (as applicable)
```

Performance budgets are defined during bootstrap (AGENTS.md) based on project requirements. The agent doesn't set the budgets — the human does. The agent's job is to stay within them.

## Conclusion

Performance is a fitness function: measurable, threshold-based, mechanically verifiable. Lighthouse CI for front-end (free, one hour of setup), k6 for APIs (free, 20-line script), size-limit for bundles (free, one JSON file). Each returns pass/fail. Each fails the build when the threshold is crossed. Each tells the agent exactly what changed. Same architecture as every other gate in the verification stack.
