## List models

```
GET {endpoint}/openai/v1/models?api-version=preview
```

Lists the currently available models, and provides basic information about each one such as the owner and availability.

### URI Parameters

| Name | In | Required | Type | Description |
| --- | --- | --- | --- | --- |
| endpoint | path | Yes | string   url | Supported Azure OpenAI endpoints (protocol and hostname, for example: `https://aoairesource.openai.azure.com`. Replace "aoairesource" with your Azure OpenAI resource name). https://{your-resource-name}.openai.azure.com |
| api-version | query | No |  | The explicit Azure AI Foundry Models API version to use for this request.   `v1` if not otherwise specified. |

### Request Header

**Use either token based authentication or API key. Authenticating with token based authentication is recommended and more secure.**

| Name | Required | Type | Description |
| --- | --- | --- | --- |
| Authorization | True | string | **Example:**`Authorization: Bearer {Azure_OpenAI_Auth_Token}`      **To generate an auth token using Azure CLI: `az account get-access-token --resource https://cognitiveservices.azure.com`**      Type: oauth2   Authorization Url: `https://login.microsoftonline.com/common/oauth2/v2.0/authorize`   scope: `https://cognitiveservices.azure.com/.default` |
| api-key | True | string | Provide Azure OpenAI API key here |

### Responses

**Status Code:** 200

**Description**: The request has succeeded.

| **Content-Type** | **Type** | **Description** |
| --- | --- | --- |
| application/json | [OpenAI.ListModelsResponse](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/#openailistmodelsresponse) |  |

**Status Code:** default

**Description**: An unexpected error response.

| **Content-Type** | **Type** | **Description** |
| --- | --- | --- |
| application/json | [AzureErrorResponse](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/#azureerrorresponse) |  |

## Retrieve model

```
GET {endpoint}/openai/v1/models/{model}?api-version=preview
```

Retrieves a model instance, providing basic information about the model such as the owner and permissioning.

### URI Parameters

| Name | In | Required | Type | Description |
| --- | --- | --- | --- | --- |
| endpoint | path | Yes | string   url | Supported Azure OpenAI endpoints (protocol and hostname, for example: `https://aoairesource.openai.azure.com`. Replace "aoairesource" with your Azure OpenAI resource name). https://{your-resource-name}.openai.azure.com |
| api-version | query | No |  | The explicit Azure AI Foundry Models API version to use for this request.   `v1` if not otherwise specified. |
| model | path | Yes | string | The ID of the model to use for this request. |

### Request Header

**Use either token based authentication or API key. Authenticating with token based authentication is recommended and more secure.**

| Name | Required | Type | Description |
| --- | --- | --- | --- |
| Authorization | True | string | **Example:**`Authorization: Bearer {Azure_OpenAI_Auth_Token}`      **To generate an auth token using Azure CLI: `az account get-access-token --resource https://cognitiveservices.azure.com`**      Type: oauth2   Authorization Url: `https://login.microsoftonline.com/common/oauth2/v2.0/authorize`   scope: `https://cognitiveservices.azure.com/.default` |
| api-key | True | string | Provide Azure OpenAI API key here |

### Responses

**Status Code:** 200

**Description**: The request has succeeded.

| **Content-Type** | **Type** | **Description** |
| --- | --- | --- |
| application/json | [OpenAI.Model](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/#openaimodel) |  |

**Status Code:** default

**Description**: An unexpected error response.

| **Content-Type** | **Type** | **Description** |
| --- | --- | --- |
| application/json | [AzureErrorResponse](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/#azureerrorresponse) |  |

## Create response

```
POST {endpoint}/openai/v1/responses?api-version=preview
```

Creates a model response.

### URI Parameters

| Name | In | Required | Type | Description |
| --- | --- | --- | --- | --- |
| endpoint | path | Yes | string   url | Supported Azure OpenAI endpoints (protocol and hostname, for example: `https://aoairesource.openai.azure.com`. Replace "aoairesource" with your Azure OpenAI resource name). https://{your-resource-name}.openai.azure.com |
| api-version | query | No |  | The explicit Azure AI Foundry Models API version to use for this request.   `v1` if not otherwise specified. |

### Request Header

**Use either token based authentication or API key. Authenticating with token based authentication is recommended and more secure.**

| Name | Required | Type | Description |
| --- | --- | --- | --- |
| Authorization | True | string | **Example:**`Authorization: Bearer {Azure_OpenAI_Auth_Token}`      **To generate an auth token using Azure CLI: `az account get-access-token --resource https://cognitiveservices.azure.com`**      Type: oauth2   Authorization Url: `https://login.microsoftonline.com/common/oauth2/v2.0/authorize`   scope: `https://cognitiveservices.azure.com/.default` |
| api-key | True | string | Provide Azure OpenAI API key here |

### Request Body

**Content-Type**: application/json

| Name | Type | Description | Required | Default |
| --- | --- | --- | --- | --- |
| background | boolean | Whether to run the model response in the background. | No | False |
| include | array | Specify additional output data to include in the model response. Currently   supported values are:   \- `code_interpreter_call.outputs`: Includes the outputs of python code execution   in code interpreter tool call items.   \- `computer_call_output.output.image_url`: Include image urls from the computer call output.   \- `file_search_call.results`: Include the search results of   the file search tool call.   \- `message.input_image.image_url`: Include image urls from the input message.   \- `message.output_text.logprobs`: Include logprobs with assistant messages.   \- `reasoning.encrypted_content`: Includes an encrypted version of reasoning   tokens in reasoning item outputs. This enables reasoning items to be used in   multi-turn conversations when using the Responses API statelessly (like   when the `store` parameter is set to `false`, or when an organization is   enrolled in the zero data retention program). | No |  |
| input | string or array |  | No |  |
| instructions | string | A system (or developer) message inserted into the model's context.      When using along with `previous_response_id`, the instructions from a previous   response will not be carried over to the next response. This makes it simple   to swap out system (or developer) messages in new responses. | No |  |
| max\_output\_tokens | integer | An upper bound for the number of tokens that can be generated for a response, including visible output tokens and reasoning tokens | No |  |
| max\_tool\_calls | integer | The maximum number of total calls to built-in tools that can be processed in a response. This maximum number applies across all built-in tool calls, not per individual tool. Any further attempts to call a tool by the model will be ignored. | No |  |
| metadata | object | Set of 16 key-value pairs that can be attached to an object. This can be   useful for storing additional information about the object in a structured   format, and querying for objects via API or the dashboard.      Keys are strings with a maximum length of 64 characters. Values are strings   with a maximum length of 512 characters. | No |  |
| model | string | The model deployment to use for the creation of this response. | Yes |  |
| parallel\_tool\_calls | boolean | Whether to allow the model to run tool calls in parallel. | No | True |
| previous\_response\_id | string | The unique ID of the previous response to the model. Use this to   create multi-turn conversations. | No |  |
| prompt | object | Reference to a prompt template and its variables. | No |  |
| └─ id | string | The unique identifier of the prompt template to use. | No |  |
| └─ variables | [OpenAI.ResponsePromptVariables](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/#openairesponsepromptvariables) | Optional map of values to substitute in for variables in your   prompt. The substitution values can either be strings, or other   Response input types like images or files. | No |  |
| └─ version | string | Optional version of the prompt template. | No |  |
| reasoning | object | **o-series models only**      Configuration options for reasoning models. | No |  |
| └─ effort | [OpenAI.ReasoningEffort](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/#openaireasoningeffort) | **o-series models only**      Constrains effort on reasoning for reasoning models.   Currently supported values are `low`, `medium`, and `high`. Reducing   reasoning effort can result in faster responses and fewer tokens used   on reasoning in a response. | No |  |
| └─ generate\_summary | enum | **Deprecated:** use `summary` instead.      A summary of the reasoning performed by the model. This can be   useful for debugging and understanding the model's reasoning process.   One of `auto`, `concise`, or `detailed`.   Possible values: `auto`, `concise`, `detailed` | No |  |
| └─ summary | enum | A summary of the reasoning performed by the model. This can be   useful for debugging and understanding the model's reasoning process.   One of `auto`, `concise`, or `detailed`.   Possible values: `auto`, `concise`, `detailed` | No |  |
| store | boolean | Whether to store the generated model response for later retrieval via   API. | No | True |
| stream | boolean | If set to true, the model response data will be streamed to the client   as it is generated using [server-sent events](https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events/Using_server-sent_events#Event_stream_format). | No | False |
| temperature | number | What sampling temperature to use, between 0 and 2. Higher values like 0.8 will make the output more random, while lower values like 0.2 will make it more focused and deterministic.   We generally recommend altering this or `top_p` but not both. | No | 1 |
| text | object | Configuration options for a text response from the model. Can be plain text or structured JSON data. | No |  |
| └─ format | [OpenAI.ResponseTextFormatConfiguration](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/#openairesponsetextformatconfiguration) |  | No |  |
| tool\_choice | object | Controls which (if any) tool is called by the model.      `none` means the model will not call any tool and instead generates a message.      `auto` means the model can pick between generating a message or calling one or   more tools.      `required` means the model must call one or more tools. | No |  |
| └─ type | [OpenAI.ToolChoiceObjectType](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/#openaitoolchoiceobjecttype) | Indicates that the model should use a built-in tool to generate a response. | No |  |
| tools | array | An array of tools the model may call while generating a response. You   can specify which tool to use by setting the `tool_choice` parameter.      The two categories of tools you can provide the model are:      \- **Built-in tools**: Tools that are provided by OpenAI that extend the   model's capabilities, like file search.   \- **Function calls (custom tools)**: Functions that are defined by you,   enabling the model to call your own code. | No |  |
| top\_logprobs | integer | An integer between 0 and 20 specifying the number of most likely tokens to return at each token position, each with an associated log probability. | No |  |
| top\_p | number | An alternative to sampling with temperature, called nucleus sampling,   where the model considers the results of the tokens with top\_p probability   mass. So 0.1 means only the tokens comprising the top 10% probability mass   are considered.      We generally recommend altering this or `temperature` but not both. | No | 1 |
| truncation | enum | The truncation strategy to use for the model response.   \- `auto`: If the context of this response and previous ones exceeds   the model's context window size, the model will truncate the   response to fit the context window by dropping input items in the   middle of the conversation.   \- `disabled` (default): If a model response will exceed the context window   size for a model, the request will fail with a 400 error.   Possible values: `auto`, `disabled` | No |  |
| user | string | A unique identifier representing your end-user, which can help OpenAI to monitor and detect abuse. | No |  |

### Responses

**Status Code:** 200

**Description**: The request has succeeded.

| **Content-Type** | **Type** | **Description** |
| --- | --- | --- |
| application/json | [AzureResponse](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/#azureresponse) |  |
| text/event-stream | [OpenAI.ResponseStreamEvent](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/#openairesponsestreamevent) |  |

**Status Code:** default

**Description**: An unexpected error response.

| **Content-Type** | **Type** | **Description** |
| --- | --- | --- |
| application/json | [AzureErrorResponse](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/#azureerrorresponse) |  |

### Examples

### Example

Create a model response

```
POST {endpoint}/openai/v1/responses?api-version=preview
```

## Get response

```
GET {endpoint}/openai/v1/responses/{response_id}?api-version=preview
```

Retrieves a model response with the given ID.

### URI Parameters

| Name | In | Required | Type | Description |
| --- | --- | --- | --- | --- |
| endpoint | path | Yes | string   url | Supported Azure OpenAI endpoints (protocol and hostname, for example: `https://aoairesource.openai.azure.com`. Replace "aoairesource" with your Azure OpenAI resource name). https://{your-resource-name}.openai.azure.com |
| api-version | query | No |  | The explicit Azure AI Foundry Models API version to use for this request.   `v1` if not otherwise specified. |
| response\_id | path | Yes | string |  |
| include\[\] | query | No | array |  |

### Request Header

**Use either token based authentication or API key. Authenticating with token based authentication is recommended and more secure.**

| Name | Required | Type | Description |
| --- | --- | --- | --- |
| Authorization | True | string | **Example:**`Authorization: Bearer {Azure_OpenAI_Auth_Token}`      **To generate an auth token using Azure CLI: `az account get-access-token --resource https://cognitiveservices.azure.com`**      Type: oauth2   Authorization Url: `https://login.microsoftonline.com/common/oauth2/v2.0/authorize`   scope: `https://cognitiveservices.azure.com/.default` |
| api-key | True | string | Provide Azure OpenAI API key here |

### Responses

**Status Code:** 200

**Description**: The request has succeeded.

| **Content-Type** | **Type** | **Description** |
| --- | --- | --- |
| application/json | [AzureResponse](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/#azureresponse) |  |

**Status Code:** default

**Description**: An unexpected error response.

| **Content-Type** | **Type** | **Description** |
| --- | --- | --- |
| application/json | [AzureErrorResponse](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/#azureerrorresponse) |  |

## Delete response

```
DELETE {endpoint}/openai/v1/responses/{response_id}?api-version=preview
```

Deletes a response by ID.

### URI Parameters

| Name | In | Required | Type | Description |
| --- | --- | --- | --- | --- |
| endpoint | path | Yes | string   url | Supported Azure OpenAI endpoints (protocol and hostname, for example: `https://aoairesource.openai.azure.com`. Replace "aoairesource" with your Azure OpenAI resource name). https://{your-resource-name}.openai.azure.com |
| api-version | query | No |  | The explicit Azure AI Foundry Models API version to use for this request.   `v1` if not otherwise specified. |
| response\_id | path | Yes | string |  |

### Request Header

**Use either token based authentication or API key. Authenticating with token based authentication is recommended and more secure.**

| Name | Required | Type | Description |
| --- | --- | --- | --- |
| Authorization | True | string | **Example:**`Authorization: Bearer {Azure_OpenAI_Auth_Token}`      **To generate an auth token using Azure CLI: `az account get-access-token --resource https://cognitiveservices.azure.com`**      Type: oauth2   Authorization Url: `https://login.microsoftonline.com/common/oauth2/v2.0/authorize`   scope: `https://cognitiveservices.azure.com/.default` |
| api-key | True | string | Provide Azure OpenAI API key here |

### Responses

**Status Code:** 200

**Description**: The request has succeeded.

| **Content-Type** | **Type** | **Description** |
| --- | --- | --- |
| application/json | object |  |

**Status Code:** default

**Description**: An unexpected error response.

| **Content-Type** | **Type** | **Description** |
| --- | --- | --- |
| application/json | [AzureErrorResponse](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/#azureerrorresponse) |  |

```
GET {endpoint}/openai/v1/responses/{response_id}/input_items?api-version=preview
```

Returns a list of input items for a given response.

### URI Parameters

| Name | In | Required | Type | Description |
| --- | --- | --- | --- | --- |
| endpoint | path | Yes | string   url | Supported Azure OpenAI endpoints (protocol and hostname, for example: `https://aoairesource.openai.azure.com`. Replace "aoairesource" with your Azure OpenAI resource name). https://{your-resource-name}.openai.azure.com |
| api-version | query | No |  | The explicit Azure AI Foundry Models API version to use for this request.   `v1` if not otherwise specified. |
| response\_id | path | Yes | string |  |
| limit | query | No | integer | A limit on the number of objects to be returned. Limit can range between 1 and 100, and the   default is 20. |
| order | query | No | string   Possible values: `asc`, `desc` | Sort order by the `created_at` timestamp of the objects. `asc` for ascending order and `desc`   for descending order. |
| after | query | No | string | A cursor for use in pagination. `after` is an object ID that defines your place in the list.   For instance, if you make a list request and receive 100 objects, ending with obj\_foo, your   subsequent call can include after=obj\_foo in order to fetch the next page of the list. |
| before | query | No | string | A cursor for use in pagination. `before` is an object ID that defines your place in the list.   For instance, if you make a list request and receive 100 objects, ending with obj\_foo, your   subsequent call can include before=obj\_foo in order to fetch the previous page of the list. |

### Request Header

**Use either token based authentication or API key. Authenticating with token based authentication is recommended and more secure.**

| Name | Required | Type | Description |
| --- | --- | --- | --- |
| Authorization | True | string | **Example:**`Authorization: Bearer {Azure_OpenAI_Auth_Token}`      **To generate an auth token using Azure CLI: `az account get-access-token --resource https://cognitiveservices.azure.com`**      Type: oauth2   Authorization Url: `https://login.microsoftonline.com/common/oauth2/v2.0/authorize`   scope: `https://cognitiveservices.azure.com/.default` |
| api-key | True | string | Provide Azure OpenAI API key here |

### Responses

**Status Code:** 200

**Description**: The request has succeeded.

| **Content-Type** | **Type** | **Description** |
| --- | --- | --- |
| application/json | [OpenAI.ResponseItemList](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/#openairesponseitemlist) |  |

**Status Code:** default

**Description**: An unexpected error response.

| **Content-Type** | **Type**                                                                                            | **Description** |
| ---------------- | --------------------------------------------------------------------------------------------------- | --------------- |
| application/json | [AzureErrorResponse](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/#azureerrorresponse) |                 |
|                  |                                                                                                     |                 |
