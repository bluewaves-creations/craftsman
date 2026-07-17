from pytest_bdd import given, parsers, scenarios, when, then

scenarios("../features/todo.feature")


@given("an empty todo list", target_fixture="todo")
def todo():
    return []


@when(parsers.parse('I add "{item}"'))
def add(todo, item):
    todo.append(item)


# NOTE: no step definition for 'I remove "..."' — on purpose (undefined-step probe).


@then(parsers.parse("the list contains {n:d} items"))
def contains(todo, n):
    assert len(todo) == n
