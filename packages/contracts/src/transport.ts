export type HttpMethod = "GET" | "POST" | "PUT" | "DELETE";

export type ContractTransportRequest = {
  operationName: string;
  request: unknown;
  method: HttpMethod;
  path: string;
  body: unknown | undefined;
  headers: Record<string, string>;
};

export type ContractCallOptions = {
  readonly signal?: AbortSignal;
};

/** Minimal protocol shared by all northbound events. */
export type TransportMessage = { type: string } & Record<string, unknown>;

export interface ContractTransport {
  send<TResponse>(
    request: ContractTransportRequest,
    options?: ContractCallOptions,
  ): Promise<TResponse>;
  stream<TEvent>(
    request: ContractTransportRequest,
    options?: ContractCallOptions,
  ): AsyncIterable<TEvent>;
  /**
   * Subscribes to backend-pushed notifications. Returns an unsubscribe function.
   */
  subscribe(handler: (message: TransportMessage) => void): () => void;
}

export type ContractStreamFrame<TEvent> =
  | { type: "data"; data: TEvent }
  | { type: "error"; error: ContractErrorPayload }
  | { type: "end" };

export type ContractErrorPayload = {
  code: string;
  message: string;
};

export type ContractErrorEnvelope = {
  error: ContractErrorPayload;
};

export class ContractTransportError extends Error {
  readonly code: string;
  readonly status: number | null;
  readonly responseBody: unknown;

  constructor({
    code,
    message,
    status,
    responseBody,
  }: {
    code: string;
    message: string;
    status: number | null;
    responseBody: unknown;
  }) {
    super(message);
    this.name = "ContractTransportError";
    this.code = code;
    this.status = status;
    this.responseBody = responseBody;
  }
}
