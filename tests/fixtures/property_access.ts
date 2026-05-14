class ErrorA extends Error {}
class ErrorB extends Error {}

type Result = { error: ErrorA | ErrorB };

/** @throws {ErrorA | ErrorB} */
function handle(r: Result): void {
  throw r.error;
}
