import assert from "node:assert";
import { Given, When, Then } from "@cucumber/cucumber";

let list = [];

Given("an empty todo list", function () {
  list = [];
});

When("I add {string}", function (item) {
  list.push(item);
});

// NOTE: no step definition for 'I remove {string}' — on purpose (undefined-step probe).

Then("the list contains {int} items", function (n) {
  assert.strictEqual(list.length, n);
});
