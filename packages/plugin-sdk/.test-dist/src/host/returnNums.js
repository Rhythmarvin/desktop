import { writeLine } from "../internal/writer";
/**
 * 向 stdout 写入成功响应。
 *
 * @param id     - 对应请求的 id
 * @param result - 返回值，会 JSON.stringify 后写入
 */
export async function returnNums(id, result) {
    const response = {
        jsonrpc: "2.0",
        id,
        result: result ?? null,
    };
    writeLine(JSON.stringify(response));
}
/**
 * 向 stdout 写入错误响应。
 *
 * @param id      - 对应请求的 id
 * @param code    - JSON-RPC 错误码
 * @param message - 错误描述
 */
returnNums.error = async function (id, code, message) {
    const response = {
        jsonrpc: "2.0",
        id,
        error: { code, message },
    };
    writeLine(JSON.stringify(response));
};
