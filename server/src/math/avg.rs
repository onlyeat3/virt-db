// 定义一个结构体
pub struct AveragedCollection {
    // 用于求平均值的数组
    list: Vec<i64>,
    // 当前list数组的平均值
    average: f64,
}

impl AveragedCollection {
    // 为使用方提供新建一个集合的方法
    pub fn new() -> Self {
        Self {
            list: vec![],
            average: 0 as f64,
        }
    }
    // 为使用方提供添加一个数字的方法
    pub fn add(&mut self, value: i64) {
        self.list.push(value);
        // 添加后立即计算平均数
        self.update_average();
    }

    // 为使用方提供删除最后一项的方法
    pub fn remove(&mut self) -> Option<i64> {
        let result = self.list.pop();
        match result {
            Some(value) => {
                // 删除后立即计算平均数
                self.update_average();
                Some(value)
            }
            None => None,
        }
    }

    // 为使用方提供获取当前平均数的方法
    pub fn average(&self) -> f64 {
        self.average
    }

    // 内部用于更新平均数的方法
    fn update_average(&mut self) {
        let total: i64 = self.list.iter().sum();
        self.average = total as f64 / self.list.len() as f64;
    }
}