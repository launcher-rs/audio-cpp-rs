/*
 * audio-cpp-sys — 基于 audio.cpp engine::runtime 的 C ABI 封装。
 *
 * 设计约定：
 *   - 所有结构化数据以 JSON 字符串跨过 ABI 边界（UTF-8 编码，以 \0 结尾）。
 *   - 音频采样以 float 数组跨过边界。
 *   - C++ 侧持有所有对象；句柄是不透明指针，必须用对应的 free() 释放。
 *   - 函数返回 0 表示成功，非 0 表示出错；最近一次错误信息可通过
 *     audiocpp_last_error() 获取。
 *   - audio.cpp 内部抛出的异常在本边界处被捕获，转换为错误码 + 错误信息，
 *     绝不会向 C 传播。
 */

#pragma once

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* 不透明的句柄类型：注册表 / 模型 / 会话 */
typedef struct audiocpp_registry audiocpp_registry;
typedef struct audiocpp_model audiocpp_model;
typedef struct audiocpp_session audiocpp_session;

/* ------------------------------------------------------------------ */
/* 错误与字符串                                                        */
/* ------------------------------------------------------------------ */

/* 返回最近一次错误的描述信息；若无错误则返回 NULL。
   返回的指针在下一次任何 audiocpp_* 调用前有效。 */
const char * audiocpp_last_error(void);

/* 释放由任意 audiocpp_* 函数返回的字符串。 */
void audiocpp_free_string(char * s);

/* ------------------------------------------------------------------ */
/* 注册表（Registry）                                                  */
/* ------------------------------------------------------------------ */

/* 创建默认注册表，包含所有编译进本库的模型族 loader。 */
audiocpp_registry * audiocpp_registry_default(void);

/* JSON 数组：已注册的模型族列表，例如 ["silero_vad","qwen3_asr"]。 */
int audiocpp_registry_families_json(const audiocpp_registry * reg, char ** out_json);

/* JSON 目录：所有 loader 的声明信息（模型族、任务、端点）。 */
int audiocpp_registry_loaders_json(const audiocpp_registry * reg, char ** out_json);

/* JSON 数组：所有后端可用的计算设备列表。 */
int audiocpp_registry_devices_json(char ** out_json);

/* 判断某模型族是否已编译进引擎（加载前预检，避免盲 load 失败）。*/
int audiocpp_registry_supports_family(const audiocpp_registry * reg, const char * family);

/* 预检模型：返回 metadata / capabilities / cli 选项 / 发现资产（JSON）。*/
int audiocpp_registry_inspect_json(const audiocpp_registry * reg,
                                   const char * model_path,
                                   char ** out_json);

/* 加载一个模型。model_path 必填；family_hint 可传 NULL。
   load_options_json 可传 NULL，或一个 JSON 对象（例如 {"weight_id":"..."}）。 */
audiocpp_model * audiocpp_registry_load(const audiocpp_registry * reg,
                                        const char * model_path,
                                        const char * family_hint,
                                        const char * load_options_json);

void audiocpp_registry_free(audiocpp_registry * reg);

/* ------------------------------------------------------------------ */
/* 模型（Model）                                                       */
/* ------------------------------------------------------------------ */

/* JSON 对象：已加载模型的元数据 / 能力。 */
int audiocpp_model_metadata_json(const audiocpp_model * model, char ** out_json);
int audiocpp_model_capabilities_json(const audiocpp_model * model, char ** out_json);

/* 在模型上创建一次任务会话。
   task: "vad" | "asr" | "tts" | ... ;
   mode: "offline" | "streaming";
   backend: "cpu" | "cuda" | "hip" | "vulkan" | "metal" | "best"。
   session_options_json 可传 NULL，或一个 JSON 选项键值对象。 */
audiocpp_session * audiocpp_model_create_task_session(const audiocpp_model * model,
                                                      const char * task,
                                                      const char * mode,
                                                      const char * backend,
                                                      int device,
                                                      int threads,
                                                      const char * session_options_json);

void audiocpp_model_free(audiocpp_model * model);

/* ------------------------------------------------------------------ */
/* 会话（Session）                                                     */
/* ------------------------------------------------------------------ */

void audiocpp_session_free(audiocpp_session * session);

const char * audiocpp_session_family(audiocpp_session * session);      /* 借用的指针 */
const char * audiocpp_session_task_kind(audiocpp_session * session);   /* 借用的指针 */
const char * audiocpp_session_run_mode(audiocpp_session * session);    /* 借用的指针 */

/* JSON 对象：流式策略描述（输入/输出类型、首选分块大小）。 */
int audiocpp_session_streaming_policy_json(audiocpp_session * session, char ** out_json);

/* 离线与流式会话都共用 prepare()。request_json 为 JSON 对象，携带
   audio / text / voice 输入（具体字段见 capi.cpp）。 */
int audiocpp_session_prepare(audiocpp_session * session, const char * request_json);

/* 离线：执行一次请求，返回 TaskResult 的 JSON。 */
int audiocpp_session_run_offline(audiocpp_session * session,
                                 const char * request_json,
                                 char ** out_json);

/* 流式事件回调。cb 可传 NULL 以清除。
   回调由 C++ 侧以 JSON StreamEvent 内容触发；回调内不得再调用回会话。
   user_data 原样透传。 */
typedef void (*audiocpp_stream_event_cb)(void * user_data, const char * event_json, int is_final);

void audiocpp_session_set_event_sink(audiocpp_session * session,
                                     audiocpp_stream_event_cb cb,
                                     void * user_data);

/* 开始一次流式会话（先 prepare 再进入流式读取循环）。 */
int audiocpp_session_start(audiocpp_session * session, const char * request_json);

/* 将一段音频送入流式会话；返回该块触发的 StreamEvent 的 JSON。 */
int audiocpp_session_process_audio(audiocpp_session * session,
                                   const float * samples,
                                   size_t count,
                                   int sample_rate,
                                   int channels,
                                   int64_t start_sample,
                                   char ** out_event_json);

/* 结束流式会话，返回最终 TaskResult 的 JSON。 */
int audiocpp_session_finish(audiocpp_session * session, char ** out_json);

/* 重置流式会话内部状态（可复用会话对象重新开始）。 */
void audiocpp_session_reset(audiocpp_session * session);

/* ------------------------------------------------------------------ */
/* 音频辅助函数（便捷封装）                                            */
/* ------------------------------------------------------------------ */

/* 将 RIFF/WAVE 文件读为 float 采样（取值范围 -1..1）。
   调用方持有返回的缓冲区，必须用 audiocpp_audio_free 释放。 */
int audiocpp_audio_load_wav(const char * path, int * sample_rate, int * channels,
                            size_t * count, float ** samples);
void audiocpp_audio_free(float * samples);

#ifdef __cplusplus
}
#endif