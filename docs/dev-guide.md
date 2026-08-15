# 开发指南
## 结构
```
/
|-- build    # 构建
|-- config   # 配置
├── docs     # 文档
|-- drivers  # 驱动
|-- examples # 示例代码
|-- include  # 头文件
|-- kernel   # 内核
|-- lib      # 库文件
|-- misc     # 杂项
|-- platform # 平台
|-- service  # 服务
```
## 函数约定
### kernel
#### init
* 函数以init_开头，包装对应模块初始化函数
#### tests
* 函数以run_开头，包装对应模块测试函数
* 在run_函数中输出测试结果，遵循`[结果] 测试名：信息`格式