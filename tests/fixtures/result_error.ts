class AnalyzerError extends Error {
  readonly code = 'ANALYZER_ERROR';
}

class ContractError extends Error {
  readonly code = 'CONTRACT_ERROR';
}

type ResultType<T, E> = { ok: true; value: T } | { ok: false; error: E };

const err = <E>(error: E): ResultType<never, E> => ({ ok: false, error });

function analyze(): ResultType<string, AnalyzerError> {
  return err(new AnalyzerError('failed'));
}

/** @throws {AnalyzerError} */
function run(): string {
  const result = analyze();
  if (!result.ok) {
    throw result.error;
  }
  return result.value;
}
